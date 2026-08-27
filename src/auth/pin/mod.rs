//! PIN-based unlock: PAM backend, config enforcement, lockout policy.

mod enforce;
mod gate;
mod pam;
mod policy;

pub use enforce::{SERVICE_NAME, pin_listed, validate_installed};
pub use gate::Gate;
pub use pam::authenticate;
pub use policy::Policy;
