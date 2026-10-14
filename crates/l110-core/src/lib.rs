//! l110-core: โครงกลางที่ทุกคำสั่งใช้ร่วมกัน
//!
//! - [`spec`]: `CommandSpec` / `OptSpec` นิยาม flag, คำอธิบาย, ตัวอย่างของคำสั่ง (แหล่งความจริงเดียว)
//! - [`getopt`]: parser พฤติกรรมเทียบเท่า GNU `getopt_long`
//! - [`help`]: สร้าง `--help` / `--version` จาก `CommandSpec`
//! - [`ctx`]: `Ctx` (stdio, env, ชื่อโปรแกรม) และ `ExitCode`
//! - [`registry`]: ตาราง applet กลาง
//! - [`error`]: error type และข้อความ error แบบ strerror

pub mod ctx;
pub mod error;
pub mod getopt;
pub mod help;
pub mod registry;
pub mod spec;

pub use ctx::{Capture, Ctx, ExitCode};
pub use error::{CmdError, strerror};
pub use getopt::{ParseError, Parsed};
pub use registry::{Applet, Args, Registry};
pub use spec::{ArgKind, CommandSpec, Example, OptSpec, ParseStyle};

/// เวอร์ชันของ L110 (ใช้ร่วมกันทั้ง workspace)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
