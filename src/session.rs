use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

use io_ring::Ring;
use wayland::{
    ExtSessionLockManagerV1, ExtSessionLockSurfaceV1, ExtSessionLockSurfaceV1Event,
    ExtSessionLockV1, ExtSessionLockV1Event, Handle, Interface, ObjectId, WlOutput,
    WlRegistryEvent,
};
use window_manager::{Color, Surface, WindowManager, prelude::*};

use crate::events::{PinOffer, PinOutcome};
use crate::widgets::Lockscreen;

const BG: Color = Color::from_rgb8(0, 0, 0);

enum Panel {
    Waiting,
    Armed(Handle<WlOutput>),
    Ready {
        window: WindowHandle<Lockscreen>,
        wl_surface_id: ObjectId,
        lock_surface_id: ObjectId,
    },
}

/// Owns the single lock surface and the ext-session-lock exit handshake.
pub(crate) struct Session {
    manager: Option<Handle<ExtSessionLockManagerV1>>,
    lock: Option<Handle<ExtSessionLockV1>>,
    is_locked: bool,
    exiting: bool,
    open: bool,
    pin_len: usize,
    offer: PinOffer,
    panel: Panel,
}

impl Session {
    pub(crate) fn new(open: bool, pin_len: usize, offer: PinOffer) -> Self {
        Self {
            manager: None,
            lock: None,
            is_locked: false,
            exiting: false,
            open,
            pin_len,
            offer,
            panel: Panel::Waiting,
        }
    }

    pub(crate) fn on_registry(
        &mut self,
        ev: &WlRegistryEvent,
        wm: &mut WindowManager,
        status: PinOutcome,
    ) {
        match ev {
            WlRegistryEvent::Global {
                sender,
                name,
                interface,
                version,
            } => {
                if self.exiting {
                    return;
                }
                match interface.as_str() {
                    ExtSessionLockManagerV1::NAME => {
                        if self.manager.is_none() {
                            self.manager = Some(sender.bind(*name, *version));
                            if let Panel::Armed(output) =
                                std::mem::replace(&mut self.panel, Panel::Waiting)
                                && let Err(e) = self.ensure_output(output, wm, status)
                            {
                                tracing::error!("failed to create lock surface: {e}");
                            }
                        }
                    }
                    WlOutput::NAME => {
                        let output = sender.bind(*name, *version);
                        if let Err(e) = self.ensure_output(output, wm, status) {
                            tracing::error!("failed to create lock surface: {e}");
                        }
                    }
                    _ => {}
                }
            }
            // The physical panel is assumed permanent.
            WlRegistryEvent::GlobalDelete { .. } => {}
        }
    }

    fn ensure_output(
        &mut self,
        output: Handle<WlOutput>,
        wm: &mut WindowManager,
        status: PinOutcome,
    ) -> Result<(), &'static str> {
        if self.exiting || !matches!(self.panel, Panel::Waiting) {
            return Ok(());
        }
        let Some(manager) = self.manager.clone() else {
            self.panel = Panel::Armed(output);
            return Ok(());
        };

        let lock = self.lock.get_or_insert_with(|| manager.lock()).clone();
        let surface = wm.create_surface();
        let wl_surface_id = surface
            .object_id()
            .ok_or("fresh surface has no object id")?;
        let lock_surface = lock.get_lock_surface(&surface, &output);
        let lock_surface_id = lock_surface
            .object_id()
            .ok_or("fresh lock role has no object id")?;

        let window = wm.spawn_window_with(
            0,
            0,
            BG,
            Lockscreen::new(self.pin_len, Rc::clone(&self.offer), self.open),
            surface,
            Box::new(SessionLockSurface { lock_surface }),
        );
        window.set(status, wm);
        self.panel = Panel::Ready {
            window,
            wl_surface_id,
            lock_surface_id,
        };
        Ok(())
    }

    pub(crate) fn route_configure(
        &self,
        ev: &ExtSessionLockSurfaceV1Event,
        wm: &mut WindowManager,
    ) {
        let ExtSessionLockSurfaceV1Event::Configure {
            sender,
            serial,
            width,
            height,
        } = ev;
        if let Some(id) = sender.object_id()
            && let Panel::Ready {
                window,
                lock_surface_id,
                ..
            } = &self.panel
            && *lock_surface_id == id
        {
            wm.configure(window.id(), *serial, *width, *height);
        }
    }

    pub(crate) fn on_lock(
        &mut self,
        ev: &ExtSessionLockV1Event,
        ring: &mut Ring,
        wm: &mut WindowManager,
    ) {
        match ev {
            ExtSessionLockV1Event::Locked { .. } => self.is_locked = true,
            ExtSessionLockV1Event::Finished { .. } => {
                tracing::info!("session lock finished; exiting");
                self.request_exit(ring, wm);
            }
        }
    }

    /// Unlock, then block until the compositor receipt (or 2s timeout).
    pub(crate) fn request_exit(&mut self, ring: &mut Ring, wm: &mut WindowManager) {
        const ACK_TIMEOUT: Duration = Duration::from_secs(2);
        if self.exiting {
            return;
        }
        self.exiting = true;
        if let Panel::Ready { window, .. } = &self.panel {
            wm.destroy(window.id());
        }
        self.panel = Panel::Waiting;
        if let Some(lock) = self.lock.take() {
            if self.is_locked {
                lock.unlock_and_destroy();
            } else {
                lock.destroy();
            }
        }
        if !wm.wayland().wait_for_ack(ring, ACK_TIMEOUT) {
            tracing::warn!("compositor did not acknowledge unlock within 2s");
        }
        std::process::exit(0);
    }

    pub(crate) fn focused(&self, wm: &WindowManager) -> Option<WindowHandle<Lockscreen>> {
        let focused = wm.current_pointer_window()?;
        match &self.panel {
            Panel::Ready { window, .. } if window.id() == focused => Some(*window),
            _ => None,
        }
    }

    pub(crate) fn window_for_touch(&self, surface: ObjectId) -> Option<WindowHandle<Lockscreen>> {
        match &self.panel {
            Panel::Ready {
                window,
                wl_surface_id,
                ..
            } if *wl_surface_id == surface => Some(*window),
            _ => None,
        }
    }

    pub(crate) fn window(&self) -> Option<WindowHandle<Lockscreen>> {
        match &self.panel {
            Panel::Ready { window, .. } => Some(*window),
            _ => None,
        }
    }
}

struct SessionLockSurface {
    lock_surface: Handle<ExtSessionLockSurfaceV1>,
}

impl Surface for SessionLockSurface {
    fn ack_configure(&self, serial: u32) {
        self.lock_surface.ack_configure(serial);
    }

    fn destroy(&mut self) {
        self.lock_surface.destroy();
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
