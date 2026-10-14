//! error type ของคำสั่งและข้อความ error แบบ `strerror`

use std::fmt;
use std::io;

use crate::ctx::ExitCode;

/// ข้อความ error ของ OS แบบ C `strerror` (ไม่มี `(os error N)` ต่อท้ายแบบ Rust)
pub fn strerror(e: &io::Error) -> String {
    let s = e.to_string();
    match s.rfind(" (os error ") {
        Some(p) if s.ends_with(')') => s[..p].to_string(),
        _ => s,
    }
}

/// error ที่คำสั่งรายงานออก stderr เป็น `prog: ข้อความ` พร้อม exit code
#[derive(Debug)]
pub struct CmdError {
    pub message: String,
    pub code: ExitCode,
}

impl CmdError {
    /// error ทั่วไป (exit 1)
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: ExitCode::FAILURE,
        }
    }

    pub fn with_code(mut self, code: ExitCode) -> Self {
        self.code = code;
        self
    }

    /// error จาก I/O พร้อมบริบท เช่น `cannot open 'x': No such file or directory`
    pub fn io(context: impl fmt::Display, e: &io::Error) -> Self {
        Self::new(format!("{context}: {}", strerror(e)))
    }
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CmdError {}

impl From<io::Error> for CmdError {
    fn from(e: io::Error) -> Self {
        Self::new(strerror(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strerror_strips_os_error_suffix() {
        let e = io::Error::from_raw_os_error(2);
        assert_eq!(strerror(&e), "No such file or directory");
        let e = io::Error::other("custom");
        assert_eq!(strerror(&e), "custom");
    }

    #[test]
    fn io_context() {
        let e = io::Error::from_raw_os_error(13);
        assert_eq!(
            CmdError::io("cannot open 'x'", &e).message,
            "cannot open 'x': Permission denied"
        );
    }
}
