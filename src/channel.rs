//! One-byte result channel bridging worker threads into the event loop.

use std::cell::Cell;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Arc;

use io_ring::{IoToken, RingProxy};

/// Read end of the result pipe, watched by the event loop's io_uring.
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

    /// Arms one io_uring read of a single byte.
    pub fn rearm(&mut self, proxy: &RingProxy) {
        // Heap buffer: its address must stay valid for the queued read.
        let ptr = Box::as_mut_ptr(&mut self.buf);
        let sqe =
            io_uring::opcode::Read::new(io_uring::types::Fd(self.fd.as_raw_fd()), ptr, 1).build();
        self.token = proxy.push(sqe);
    }

    /// Consumes the completion for the armed read; stale tokens are ignored.
    pub fn take(&mut self, proxy: &RingProxy, token: &IoToken, nread: i32) -> Option<u8> {
        if self.token.0 != token.0 {
            return None;
        }
        if nread < 1 {
            tracing::error!("result pipe closed early (nread={nread})");
        }
        let byte = if nread < 1 { 0 } else { *self.buf };
        self.rearm(proxy);
        Some(byte)
    }
}

/// Write end of the result pipe; spawns one worker per submission.
pub struct Trigger {
    write_fd: OwnedFd,
    in_flight: Cell<bool>,
    work: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}

impl Trigger {
    pub fn new(write_fd: OwnedFd, work: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        Self {
            write_fd,
            in_flight: Cell::new(false),
            work: Arc::new(work),
        }
    }

    /// Marks the current submission as fully processed.
    pub fn finish(&self) {
        self.in_flight.set(false);
    }

    /// Spawns a worker for `input`; its verdict is written as one byte
    /// (`1`/`0`). Returns false if a submission is already in flight.
    pub fn submit(&self, input: &str) -> bool {
        if self.in_flight.get() {
            return false;
        }
        self.in_flight.set(true);
        let Ok(write_fd) = self.write_fd.try_clone() else {
            tracing::error!("failed to clone result pipe fd");
            self.in_flight.set(false);
            return false;
        };
        let input = input.to_string();
        let work = self.work.clone();
        std::thread::spawn(move || {
            let byte = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&input)))
            {
                Ok(ok) => u8::from(ok),
                Err(_) => {
                    tracing::error!("worker panicked");
                    0
                }
            };
            if !write_byte(&write_fd, byte) {
                tracing::error!("result pipe write failed");
            }
        });
        true
    }
}

/// Writes one byte, retrying on interruption.
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
    fn write_byte_sends_one() -> Result<(), std::io::Error> {
        let (read_fd, write_fd) = rustix::pipe::pipe()?;
        assert!(write_byte(&write_fd, 1));
        let mut buf = [0u8; 1];
        let n = rustix::io::read(&read_fd, &mut buf)?;
        assert_eq!(n, 1);
        assert_eq!(buf[0], 1);
        Ok(())
    }
}
