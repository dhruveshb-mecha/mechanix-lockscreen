use std::cell::RefCell;
use std::rc::Rc;

use app::Event;

use utils::Point;

pub type PinOffer = Rc<RefCell<Option<String>>>;

#[derive(Debug, Clone, Copy)]
pub struct PinClick(pub Point);
impl Event for PinClick {}

#[derive(Debug)]
pub struct PinRelease;
impl Event for PinRelease {}

#[derive(Debug, Clone, Copy)]
pub enum PinOutcome {
    Checking,
    Incorrect,
    LockedOut {
        secs: u32,
    },
    /// PAM stack rejected at startup.
    Unavailable,
    Prompt,
}
impl Event for PinOutcome {}
