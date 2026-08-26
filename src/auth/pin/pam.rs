use std::cell::RefCell;
use std::ffi::{OsStr, OsString};

use nonstick::{
    AuthnFlags, ConversationAdapter, ErrorCode, Result as PamResult, Transaction,
    TransactionBuilder,
};

use super::enforce::SERVICE_NAME;

pub struct PinConversation {
    pin: RefCell<Option<String>>,
}

impl PinConversation {
    pub fn new(pin: String) -> Self {
        Self {
            pin: RefCell::new(Some(pin)),
        }
    }
}

impl ConversationAdapter for PinConversation {
    fn prompt(&self, request: impl AsRef<OsStr>) -> PamResult<OsString> {
        tracing::debug!("unexpected PAM echo prompt: {:?}", request.as_ref());
        Err(ErrorCode::ConversationError)
    }

    fn masked_prompt(&self, _request: impl AsRef<OsStr>) -> PamResult<OsString> {
        self.pin
            .borrow_mut()
            .take()
            .map(OsString::from)
            .ok_or(ErrorCode::ConversationError)
    }

    fn info_msg(&self, message: impl AsRef<OsStr>) {
        tracing::info!("[pam] {}", message.as_ref().to_string_lossy());
    }

    fn error_msg(&self, message: impl AsRef<OsStr>) {
        tracing::warn!("[pam] {}", message.as_ref().to_string_lossy());
    }
}

pub fn authenticate(username: &str, pin: &str) -> Result<(), String> {
    let started = std::time::Instant::now();
    let result = authenticate_inner(username, pin);
    tracing::info!("PAM transaction took {:?}", started.elapsed());
    result
}

fn authenticate_inner(username: &str, pin: &str) -> Result<(), String> {
    let conversation = PinConversation::new(pin.to_string());

    let mut txn = TransactionBuilder::new_with_service(SERVICE_NAME)
        .username(username)
        .build(conversation.into_conversation())
        .map_err(|e| format!("PAM setup for service '{SERVICE_NAME}' failed: {e}"))?;

    txn.authenticate(AuthnFlags::empty())
        .map_err(|e| format!("authentication failed: {e}"))?;

    Ok(())
}
