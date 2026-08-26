//! The authentication result channel.
//!
//! Both ends of an OS pipe that delivers PAM verdicts from worker threads back
//! to the single-threaded event loop:
//!
//! - [`Trigger`] holds the write end: it spawns the PAM worker and enforces a
//!   single in-flight attempt.
//! - [`ResultChannel`] holds the read end: it keeps an io_uring read armed so
//!   the event loop is woken exactly when a verdict arrives.
//!
//! Protocol: every completed attempt contributes exactly one byte — `1` for a
//! successful authentication, `0` for failure or panic. The single-flight
//! invariant makes ownership of each byte unambiguous.

use std::cell::Cell;
use std::os::fd::{AsRawFd, OwnedFd};

use io_ring::{IoToken, RingProxy};

use super::pam;

/// Read end of the result pipe, registered with the event loop's io_uring.
pub struct ResultChannel {
    fd: OwnedFd,
    token: IoToken,
    buf: Box<u8>,
}

impl ResultChannel {
    pub fn new(read_fd: OwnedFd, proxy: RingProxy) -> Self {
        let mut channel = Self {
            fd: read_fd,
            token: IoToken(0),
            buf: Box::new(0u8),
        };
        channel.rearm(&proxy);
        channel
    }

    /// Arms one io_uring read of a single verdict byte.
    pub fn rearm(&mut self, proxy: &RingProxy) {
        // The buffer is heap-allocated so its address stays valid for the
        // lifetime of the queued read, even as `self` moves around.
        let ptr = Box::as_mut_ptr(&mut self.buf);
        let sqe =
            io_uring::opcode::Read::new(io_uring::types::Fd(self.fd.as_raw_fd()), ptr, 1).build();
        self.token = proxy.push(sqe);
    }

    /// Consumes the completion for the currently armed read, returning the
    /// verdict byte. Completions for stale tokens are ignored (`None`).
    pub fn take(&mut self, proxy: &RingProxy, token: &IoToken, nread: i32) -> Option<u8> {
        if self.token.0 != token.0 {
            return None;
        }
        if nread < 1 {
            tracing::error!("auth result pipe closed early (nread={nread})");
        }
        let byte = if nread < 1 { 0 } else { *self.buf };
        self.rearm(proxy);
        Some(byte)
    }
}

/// Write end of the result pipe; spawns one PAM worker per attempt.
pub struct Trigger {
    username: String,
    write_fd: OwnedFd,
    in_flight: Cell<bool>,
}

impl Trigger {
    pub fn new(username: String, write_fd: OwnedFd) -> Self {
        Self {
            username,
            write_fd,
            in_flight: Cell::new(false),
        }
    }

    /// Marks the current attempt as fully processed, allowing the next one.
    pub fn finish(&self) {
        self.in_flight.set(false);
    }

    pub fn is_busy(&self) -> bool {
        self.in_flight.get()
    }

    /// Spawns a PAM worker for this PIN. Returns false if an attempt is
    /// already in flight or the pipe cannot be used.
    pub fn submit(&self, pin: &str) -> bool {
        if self.in_flight.get() {
            return false;
        }
        self.in_flight.set(true);
        let Ok(write_fd) = self.write_fd.try_clone() else {
            tracing::error!("failed to clone auth result pipe fd");
            self.in_flight.set(false);
            return false;
        };
        let username = self.username.clone();
        let pin = pin.to_string();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pam::authenticate(&username, &pin)
            }));
            let byte = match result {
                Ok(Ok(())) => 1,
                Ok(Err(err)) => {
                    tracing::warn!("PAM authentication failed: {err}");
                    0
                }
                Err(_) => {
                    tracing::error!("PAM authentication panicked");
                    0
                }
            };
            if !write_byte(&write_fd, byte) {
                tracing::error!("auth result pipe write failed");
            }
            // `write_fd` closes automatically here via OwnedFd drop.
        });
        true
    }
}

/// Writes exactly one verdict byte, retrying on interruption.
fn write_byte(fd: &OwnedFd, byte: u8) -> bool {
    loop {
        match rustix::io::write(fd, &[byte]) {
            Ok(1) => return true,
            Ok(_) => return false,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_byte_sends_one() {
        let (read_fd, write_fd) = rustix::pipe::pipe().unwrap();
        assert!(write_byte(&write_fd, 1));
        let mut buf = [0u8; 1];
        let n = rustix::io::read(&read_fd, &mut buf).unwrap();
        assert_eq!(n, 1);
        assert_eq!(buf[0], 1);
    }
}
