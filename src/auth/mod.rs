pub mod pin;

use app::Event;

/// Owner of the locked session. Every unlock method should use this identity.
pub fn session_user() -> Option<String> {
    // SAFETY: geteuid is always safe; getpwuid_r is the thread-safe lookup.
    // This is the only direct libc call left: rustix covers pipe/read/write,
    // but deliberately provides no passwd database lookups.
    unsafe {
        let uid = libc::geteuid();
        let mut pwd = std::mem::zeroed::<libc::passwd>();
        let mut buf = vec![0 as libc::c_char; 4096];
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        let rc = libc::getpwuid_r(uid, &mut pwd, buf.as_mut_ptr(), buf.len(), &mut result);
        if rc != 0 || result.is_null() {
            return None;
        }
        let name = std::ffi::CStr::from_ptr(pwd.pw_name)
            .to_string_lossy()
            .into_owned();
        Some(name)
    }
}

#[derive(Debug)]
pub struct AuthFinished(pub bool);
impl Event for AuthFinished {}
