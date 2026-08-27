use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

use io_ring::Ring;
use wayland::{
    ExtSessionLockManagerV1, ExtSessionLockSurfaceV1, ExtSessionLockSurfaceV1Event,
    ExtSessionLockV1, ExtSessionLockV1Event, Handle, Interface, WlOutput, WlRegistryEvent,
    WlSurface,
};
use window_manager::{Color, Surface, WindowManager, prelude::*};

use crate::events::{PinOffer, PinOutcome};
use crate::widgets::Lockscreen;

const BG: Color = Color::from_rgb8(20, 20, 21);

enum LockSurface {
    Waiting,
    Armed(Handle<WlOutput>),
    Ready {
        window: WindowHandle<Lockscreen>,
        wl_surface: Handle<WlSurface>,
        lock_surface: Handle<ExtSessionLockSurfaceV1>,
    },
}

/// Owns the lock surface and the ext-session-lock exit handshake.
pub(crate) struct Session {
    manager: Option<Handle<ExtSessionLockManagerV1>>,
    lock: Option<Handle<ExtSessionLockV1>>,
    is_locked: bool,
    exiting: bool,
    open: bool,
    pin_len: usize,
    offer: PinOffer,
    lock_surface: LockSurface,
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
            lock_surface: LockSurface::Waiting,
        }
    }

    pub(crate) fn on_registry(
        &mut self,
        ev: &WlRegistryEvent,
        wm: &mut WindowManager,
        status: PinOutcome,
    ) {
        if self.exiting {
            return;
        }
        let WlRegistryEvent::Global {
            sender,
            name,
            interface,
            version,
        } = ev
        else {
            return;
        };
        match interface.as_str() {
            ExtSessionLockManagerV1::NAME => {
                if self.manager.is_none() {
                    self.manager = Some(sender.bind(*name, *version));
                    if let LockSurface::Armed(output) =
                        std::mem::replace(&mut self.lock_surface, LockSurface::Waiting)
                    {
                        self.create_lock_surface(output, wm, status);
                    }
                }
            }
            WlOutput::NAME => {
                let output = sender.bind(*name, *version);
                self.create_lock_surface(output, wm, status);
            }
            _ => {}
        }
    }

    fn create_lock_surface(
        &mut self,
        output: Handle<WlOutput>,
        wm: &mut WindowManager,
        status: PinOutcome,
    ) {
        let LockSurface::Waiting = self.lock_surface else {
            return;
        };
        let Some(manager) = self.manager.clone() else {
            self.lock_surface = LockSurface::Armed(output);
            return;
        };

        let lock = self.lock.get_or_insert_with(|| manager.lock()).clone();
        let surface = wm.create_surface();
        let lock_surface = lock.get_lock_surface(&surface, &output);
        let window = wm.spawn_window_with(
            0,
            0,
            BG,
            Lockscreen::new(self.pin_len, Rc::clone(&self.offer), self.open),
            surface.clone(),
            Box::new(SessionLockSurface {
                lock_surface: lock_surface.clone(),
            }),
        );
        window.set(status, wm);
        self.lock_surface = LockSurface::Ready {
            window,
            wl_surface: surface,
            lock_surface,
        };
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
        if let LockSurface::Ready {
            window,
            lock_surface,
            ..
        } = &self.lock_surface
            && sender == lock_surface
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
        if let LockSurface::Ready { window, .. } = &self.lock_surface {
            wm.destroy(window.id());
        }
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
        self.window().filter(|w| w.id() == focused)
    }

    pub(crate) fn window_for_touch(
        &self,
        surface: &Handle<WlSurface>,
    ) -> Option<WindowHandle<Lockscreen>> {
        match &self.lock_surface {
            LockSurface::Ready {
                window, wl_surface, ..
            } if wl_surface == surface => Some(*window),
            _ => None,
        }
    }

    pub(crate) fn window(&self) -> Option<WindowHandle<Lockscreen>> {
        match &self.lock_surface {
            LockSurface::Ready { window, .. } => Some(*window),
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
