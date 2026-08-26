//! PIN-based unlock: PAM backend, config enforcement, lockout policy, and the
//! result channel bridging worker threads into the event loop.

mod channel;
mod enforce;
mod gate;
mod pam;
mod policy;

pub use channel::{ResultChannel, Trigger};
pub use enforce::{SERVICE_NAME, pin_listed, validate_installed};
pub use gate::Gate;
pub use policy::Policy;
