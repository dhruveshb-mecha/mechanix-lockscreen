pub mod atlas {
    include!(concat!(env!("OUT_DIR"), "/lockscreen_gen.rs"));
}

pub mod auth;
mod channel;
pub mod events;
mod session;
pub mod widgets;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use app::prelude::*;
use app::{Poll, PrePoll, Start};
use io_ring::{IoEvent, Ring};
use timer::{Absolute, Clock, Relative, Timer, TimerEvent, TimerId};
use tracing_subscriber::{EnvFilter, filter::LevelFilter};
use ui::Point;
use wayland::{
    ExtSessionLockSurfaceV1Event, ExtSessionLockV1Event, WlPointerButtonState, WlPointerEvent,
    WlRegistryEvent, WlTouchEvent,
};
use window_manager::prelude::*;

use auth::AuthFinished;
use auth::pin::{self, Gate, Policy};
use channel::{ResultChannel, Trigger};
use events::{PinClick, PinOffer, PinOutcome, PinRelease};
use session::Session;
use widgets::{DateTime, DateTimeChanged, DateTimeTick, DateTimeUpdate, Lockscreen};

#[derive(Debug, Clone, Copy)]
enum AuthResult {
    Unlocked,
    Displayed(PinOutcome),
}

/// Auth orchestration: verdict channel, attempt gate, and the widget→app
/// PIN hand-off slot.
struct PinAuth {
    policy: Policy,
    gate: Gate,
    available: bool,
    rx: ResultChannel,
    trigger: Trigger,
    pending: PinOffer,
    // Wake-up hint only; the Gate decides when the lockout is over.
    lockout_timer: Option<TimerId>,
}

impl PinAuth {
    fn new(
        policy: Policy,
        username: String,
        available: bool,
        read_fd: std::os::fd::OwnedFd,
        write_fd: std::os::fd::OwnedFd,
        offer: PinOffer,
        proxy: io_ring::RingProxy,
    ) -> Self {
        let trigger = Trigger::new(write_fd, move |pin: &str| {
            pin::authenticate(&username, pin)
                .inspect_err(|e| tracing::warn!("PAM authentication failed: {e}"))
                .is_ok()
        });
        Self {
            gate: Gate::new(policy.max_attempts, policy.lockout),
            policy,
            available,
            rx: ResultChannel::new(read_fd, proxy),
            trigger,
            pending: offer,
            lockout_timer: None,
        }
    }

    fn submit_pending(&mut self, now: Instant) -> Option<PinOutcome> {
        let pin = self.pending.borrow_mut().take()?;
        if !self.available || self.gate.lockout_remaining(now).is_some() {
            return None;
        }
        self.trigger.submit(&pin).then_some(PinOutcome::Checking)
    }

    fn take_verdict(&mut self, ev: &IoEvent, ring: &Ring) -> Option<bool> {
        let IoEvent::Completed { token, result } = ev;
        self.rx
            .take(&ring.proxy(), token, *result)
            .map(|byte| byte == 1)
    }

    fn resolve(&mut self, ok: bool, now: Instant, timer: &mut Timer) -> AuthResult {
        self.trigger.finish();
        if ok {
            tracing::info!("authentication succeeded; unlocking");
            self.gate.succeeded();
            return AuthResult::Unlocked;
        }

        tracing::warn!("authentication failed");
        if let Some(secs) = self.gate.lockout_remaining(now) {
            return AuthResult::Displayed(PinOutcome::LockedOut { secs });
        }
        if self.gate.failed(now).is_some() {
            let lockout = self.policy.lockout;
            tracing::warn!("lockout engaged for {lockout:?}");
            self.lockout_timer = Some(timer.start_timer(Relative {
                duration: lockout,
                repeat: false,
            }));
            return AuthResult::Displayed(PinOutcome::LockedOut {
                secs: lockout.as_secs() as u32,
            });
        }
        AuthResult::Displayed(PinOutcome::Incorrect)
    }

    /// Whether this completion was our outstanding lockout wake-up.
    fn lockout_wake(&mut self, ev: &TimerEvent) -> bool {
        let TimerEvent::Finished { id } = ev else {
            return false;
        };
        if self.lockout_timer == Some(*id) {
            self.lockout_timer = None;
            true
        } else {
            false
        }
    }

    fn lockout_expired(&mut self) -> bool {
        self.gate.tick(Instant::now())
    }
}

#[derive(State)]
struct LockscreenApp {
    ring: Ring,
    wm: WindowManager,
    timer: Timer,
    datetime: DateTime,
    #[lens(skip)]
    auth: PinAuth,
    #[lens(skip)]
    session: Session,
    #[lens(skip)]
    status: PinOutcome,
    #[lens(skip)]
    open: bool,
    #[lens(skip)]
    pointer: Point,
}

fn on_registry(s: &mut LockscreenApp, ev: &WlRegistryEvent) {
    s.session.on_registry(ev, &mut s.wm, s.status);
}

fn on_configure(s: &mut LockscreenApp, ev: &ExtSessionLockSurfaceV1Event) {
    s.session.route_configure(ev, &mut s.wm);
}

fn on_lock(s: &mut LockscreenApp, ev: &ExtSessionLockV1Event) {
    s.session.on_lock(ev, &mut s.ring, &mut s.wm);
}

/// Click delivery may complete a PIN; drain the offer right after.
fn dispatch_press(s: &mut LockscreenApp, point: Point, handle: WindowHandle<Lockscreen>) {
    if s.open {
        request_exit(s);
        return;
    }
    handle.set(PinClick(point), &mut s.wm);
    take_pin(s);
}

fn take_pin(s: &mut LockscreenApp) {
    if let Some(outcome) = s.auth.submit_pending(Instant::now()) {
        set_outcome(s, outcome);
    }
}

fn on_pointer(s: &mut LockscreenApp, ev: &WlPointerEvent) {
    match ev {
        WlPointerEvent::Enter {
            surface_x,
            surface_y,
            ..
        }
        | WlPointerEvent::Motion {
            surface_x,
            surface_y,
            ..
        } => {
            s.pointer = Point::new(*surface_x, *surface_y);
        }
        WlPointerEvent::Button {
            state: WlPointerButtonState::Pressed,
            ..
        } => {
            if let Some(handle) = s.session.focused(&s.wm) {
                dispatch_press(s, s.pointer, handle);
            }
        }
        WlPointerEvent::Button {
            state: WlPointerButtonState::Released,
            ..
        } => {
            if let Some(handle) = s.session.focused(&s.wm) {
                handle.set(PinRelease, &mut s.wm);
            }
        }
        _ => {}
    }
}

fn on_touch(s: &mut LockscreenApp, ev: &WlTouchEvent) {
    match ev {
        WlTouchEvent::Down { surface, x, y, .. } => {
            let Some(handle) = s.session.window_for_touch(surface) else {
                return;
            };
            dispatch_press(s, Point::new(*x, *y), handle);
        }
        WlTouchEvent::Up { .. } | WlTouchEvent::Cancel { .. } => {
            if let Some(handle) = s.session.window() {
                handle.set(PinRelease, &mut s.wm);
            }
        }
        _ => {}
    }
}

fn on_io_event(s: &mut LockscreenApp, ev: &IoEvent) -> Option<AuthFinished> {
    s.auth.take_verdict(ev, &s.ring).map(AuthFinished)
}

fn on_auth_finished(s: &mut LockscreenApp, ev: &AuthFinished) {
    match s.auth.resolve(ev.0, Instant::now(), &mut s.timer) {
        AuthResult::Unlocked => request_exit(s),
        AuthResult::Displayed(outcome) => set_outcome(s, outcome),
    }
}

fn request_exit(s: &mut LockscreenApp) {
    s.session.request_exit(&mut s.ring, &mut s.wm);
}

fn set_outcome(s: &mut LockscreenApp, outcome: PinOutcome) {
    s.status = outcome;
    if let Some(handle) = s.session.window() {
        handle.set(outcome, &mut s.wm);
    }
}

fn schedule_next_update(s: &mut LockscreenApp) {
    let next_at = s.datetime.next_deadline();
    s.timer.start_deadline(Absolute {
        at: next_at,
        clock: Clock::Realtime,
    });
}

fn on_start(s: &mut LockscreenApp, _: &Start) -> DateTimeTick {
    schedule_next_update(s);
    DateTimeTick
}

fn on_timer(s: &mut LockscreenApp, ev: &TimerEvent) -> DateTimeTick {
    if s.auth.lockout_wake(ev) {
        if s.auth.lockout_expired() {
            set_outcome(s, PinOutcome::Prompt);
        }
        return DateTimeTick;
    }
    schedule_next_update(s);
    DateTimeTick
}

fn on_datetime_changed(s: &mut LockscreenApp, _: &DateTimeChanged) {
    if s.auth.lockout_expired() {
        set_outcome(s, PinOutcome::Prompt);
    }
    let update = DateTimeUpdate {
        time: DateTime::format("%H:%M"),
        date: DateTime::format("%a %d"),
    };
    if let Some(handle) = s.session.window() {
        handle.set(update, &mut s.wm);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // RUST_LOG wins if set; otherwise default to info so auth outcomes and
    // config validation are visible on a stock device.
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let enforced = auth::pin::validate_installed();
    match &enforced {
        Ok(cfg) => tracing::info!(
            "PIN PAM stack enforced: {} (pwdfile={})",
            auth::pin::SERVICE_NAME,
            cfg.pwdfile
        ),
        Err(e) => tracing::error!("{e}; PIN authentication disabled"),
    }

    let username = auth::session_user();
    if username.is_none() {
        tracing::error!("could not resolve session user; PIN authentication disabled");
    }
    // Open mode: recipe legal, identity known, but the book has no entry for
    // it - no PIN was ever set, so a tap authorizes the unlock.
    let (available, open) = match (&enforced, username.as_deref()) {
        (Ok(cfg), Some(user)) => {
            if pin::pin_listed(&cfg.path, user) {
                (true, false)
            } else {
                tracing::info!("no PIN registered for {user}; tap to unlock");
                (true, true)
            }
        }
        _ => (false, false),
    };
    let policy = Policy::new();
    let offer: PinOffer = Rc::new(RefCell::new(None));
    let ring = Ring::default();

    let (result_rx, result_tx) = rustix::pipe::pipe().map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("pipe() failed for auth result channel: {e}"),
        )
    })?;
    let auth = PinAuth::new(
        policy,
        username.unwrap_or_default(),
        available && !open,
        result_rx,
        result_tx,
        Rc::clone(&offer),
        ring.proxy(),
    );

    let mut wm = WindowManager::new(ring.proxy());
    wm.upload_atlas(&atlas::LOCKSCREEN);
    let timer = Timer::new(ring.proxy());
    let session = Session::new(open, policy.pin_len, offer);

    let status = if available {
        PinOutcome::Prompt
    } else {
        PinOutcome::Unavailable
    };

    let state = LockscreenApp {
        auth,
        session,
        status,
        open,
        pointer: Point::new(-1.0, -1.0),
        ring,
        wm,
        timer,
        datetime: DateTime::new(),
    };

    let mut app = App::new(state)
        .mount(io_ring::module())
        .mount(window_manager::module())
        .mount(timer::module())
        .mount(DateTime::module())
        .mount(
            Module::new()
                .on(on_registry)
                .on(on_configure)
                .on(on_lock)
                .on(on_pointer)
                .on(on_touch)
                .on(on_io_event)
                .on(on_auth_finished)
                .on(on_start)
                .on(on_timer)
                .on(on_datetime_changed),
        );

    app.dispatch(&Start);
    loop {
        app.dispatch(&PrePoll);
        app.dispatch(&Poll);
    }
}
