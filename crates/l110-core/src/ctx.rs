//! `Ctx`: สภาพแวดล้อมที่ส่งให้ทุกคำสั่ง (stdio, env, ชื่อโปรแกรม)
//!
//! แยก stdio ออกมาเป็น trait object เพื่อให้ทดสอบคำสั่งใน process เดียวกันได้โดยไม่ต้อง spawn

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use crate::error::{CmdError, strerror};
use crate::getopt::{self, ParseError, Parsed};
use crate::help;
use crate::spec::CommandSpec;

/// exit status ของคำสั่ง
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitCode(pub u8);

impl ExitCode {
    pub const SUCCESS: ExitCode = ExitCode(0);
    pub const FAILURE: ExitCode = ExitCode(1);
    pub const USAGE: ExitCode = ExitCode(2);
    /// ไม่พบคำสั่ง (เหมือน shell)
    pub const NOT_FOUND: ExitCode = ExitCode(127);
}

impl From<u8> for ExitCode {
    fn from(v: u8) -> Self {
        ExitCode(v)
    }
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(c: ExitCode) -> Self {
        std::process::ExitCode::from(c.0)
    }
}

/// สภาพแวดล้อมของการรันคำสั่งหนึ่งครั้ง
pub struct Ctx {
    /// ชื่อที่ใช้นำหน้าข้อความ error (`ls: ...`)
    pub prog: String,
    pub out: Box<dyn Write>,
    pub err: Box<dyn Write>,
    pub input: Box<dyn Read>,
    env: BTreeMap<OsString, OsString>,
}

impl Ctx {
    pub fn new(
        prog: impl Into<String>,
        out: Box<dyn Write>,
        err: Box<dyn Write>,
        input: Box<dyn Read>,
        env: BTreeMap<OsString, OsString>,
    ) -> Self {
        Self {
            prog: prog.into(),
            out,
            err,
            input,
            env,
        }
    }

    /// stdio และ env ของ process จริง
    pub fn stdio(prog: impl Into<String>) -> Self {
        Self::new(
            prog,
            Box::new(io::stdout()),
            Box::new(io::stderr()),
            Box::new(io::stdin()),
            std::env::vars_os().collect(),
        )
    }

    /// Ctx สำหรับทดสอบ: จับ stdout/stderr ไว้ตรวจ, stdin ว่าง, env ตามที่ให้
    pub fn captured(prog: &str, env: &[(&str, &str)]) -> (Ctx, Capture, Capture) {
        let out = Capture::default();
        let err = Capture::default();
        let ctx = Ctx::new(
            prog,
            Box::new(out.clone()),
            Box::new(err.clone()),
            Box::new(io::empty()),
            env.iter().map(|(k, v)| (k.into(), v.into())).collect(),
        );
        (ctx, out, err)
    }

    pub fn env_get(&self, key: &str) -> Option<&OsStr> {
        self.env.get(OsStr::new(key)).map(OsString::as_os_str)
    }

    pub fn env_vars(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.env.iter().map(|(k, v)| (k.as_os_str(), v.as_os_str()))
    }

    /// `POSIXLY_CORRECT` ถูกตั้งหรือไม่ (ค่าว่างก็นับ เหมือน GNU)
    pub fn posixly_correct(&self) -> bool {
        self.env_get("POSIXLY_CORRECT").is_some()
    }

    /// พิมพ์ `prog: ข้อความ` ลง stderr
    pub fn error(&mut self, msg: impl std::fmt::Display) {
        let _ = writeln!(self.err, "{}: {}", self.prog, msg);
    }

    /// รายงาน error ของคำสั่งแล้วคืน exit code ของมัน
    pub fn report(&mut self, e: &CmdError) -> ExitCode {
        self.error(e);
        e.code
    }

    /// พิมพ์ข้อความใช้ผิดวิธีพร้อม `Try '... --help'` แล้วคืน exit code ตาม spec
    pub fn usage_error(&mut self, spec: &CommandSpec, msg: impl std::fmt::Display) -> ExitCode {
        self.error(msg);
        let _ = self.err.write_all(help::try_help_line(&self.prog).as_bytes());
        ExitCode(spec.usage_status)
    }

    pub fn print_help(&mut self, spec: &CommandSpec) {
        let _ = self.out.write_all(help::render_help(spec).as_bytes());
    }

    pub fn print_version(&mut self, spec: &CommandSpec) {
        let _ = self.out.write_all(help::render_version(spec).as_bytes());
    }

    /// parse argument ด้วย parser กลาง; จัดการ `--help`, `--version` และ error ให้
    /// `Err(code)` หมายถึงพิมพ์ข้อความเรียบร้อยแล้ว ให้คำสั่งออกด้วย code นั้นทันที
    pub fn parse(&mut self, spec: &CommandSpec, args: &[OsString]) -> Result<Parsed, ExitCode> {
        match getopt::parse(spec, args, self.posixly_correct()) {
            Ok(p) => Ok(p),
            Err(ParseError::Help) => {
                self.print_help(spec);
                Err(self.finish(ExitCode::SUCCESS))
            }
            Err(ParseError::Version) => {
                self.print_version(spec);
                Err(self.finish(ExitCode::SUCCESS))
            }
            Err(ParseError::Usage(msg)) => Err(self.usage_error(spec, msg)),
        }
    }

    /// flush stdout; ถ้าเขียนไม่สำเร็จรายงาน `write error` แล้วเปลี่ยนเป็น exit 1
    /// (เหมือน `close_stdout` ของ GNU) ทุกคำสั่งควรคืนค่าผ่านฟังก์ชันนี้
    pub fn finish(&mut self, code: ExitCode) -> ExitCode {
        match self.out.flush() {
            Ok(()) => code,
            Err(e) => {
                self.error(format!("write error: {}", strerror(&e)));
                ExitCode::FAILURE
            }
        }
    }
}

/// buffer ที่ clone ได้ ใช้จับ output ในการทดสอบ
#[derive(Clone, Default)]
pub struct Capture(Arc<Mutex<Vec<u8>>>);

impl Capture {
    pub fn bytes(&self) -> Vec<u8> {
        self.0.lock().expect("capture lock").clone()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes()).into_owned()
    }
}

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("capture lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::OptSpec;

    const SPEC: CommandSpec = CommandSpec {
        name: "demo",
        usage: &["[OPTION]..."],
        options: &[OptSpec::flag("all", Some('a'), Some("all"), "all")],
        usage_status: 2,
        ..CommandSpec::BASE
    };

    fn a(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    #[test]
    fn parse_help_prints_and_exits_zero() {
        let (mut ctx, out, _err) = Ctx::captured("demo", &[]);
        let r = ctx.parse(&SPEC, &a(&["--help"]));
        assert_eq!(r.unwrap_err(), ExitCode::SUCCESS);
        assert!(out.text().starts_with("Usage: demo"));
    }

    #[test]
    fn parse_usage_error_uses_spec_status_and_try_line() {
        let (mut ctx, out, err) = Ctx::captured("demo", &[]);
        let r = ctx.parse(&SPEC, &a(&["-z"]));
        assert_eq!(r.unwrap_err(), ExitCode(2));
        assert_eq!(out.text(), "");
        assert_eq!(
            err.text(),
            "demo: invalid option -- 'z'\nTry 'demo --help' for more information.\n"
        );
    }

    #[test]
    fn posixly_correct_comes_from_ctx_env() {
        let (ctx, _, _) = Ctx::captured("demo", &[("POSIXLY_CORRECT", "")]);
        assert!(ctx.posixly_correct());
        let (ctx, _, _) = Ctx::captured("demo", &[]);
        assert!(!ctx.posixly_correct());
    }
}
