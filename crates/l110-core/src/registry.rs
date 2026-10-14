//! ตาราง applet กลาง

use std::ffi::OsString;
use std::sync::OnceLock;

use crate::ctx::{Ctx, ExitCode};
use crate::spec::CommandSpec;

/// argument ของคำสั่ง: `argv0` แยกจากส่วนที่เหลือ
#[derive(Clone, Debug)]
pub struct Args {
    pub argv0: OsString,
    pub rest: Vec<OsString>,
}

impl Args {
    pub fn new(argv0: impl Into<OsString>, rest: Vec<OsString>) -> Self {
        Self {
            argv0: argv0.into(),
            rest,
        }
    }

    /// สร้างจาก `std::env::args_os()`
    pub fn from_env() -> Self {
        let mut it = std::env::args_os();
        let argv0 = it.next().unwrap_or_default();
        Self {
            argv0,
            rest: it.collect(),
        }
    }

    /// สร้างจากรายการสตริง (สะดวกในการทดสอบ)
    pub fn from_strs(argv0: &str, rest: &[&str]) -> Self {
        Self::new(argv0, rest.iter().map(OsString::from).collect())
    }

    pub fn rest(&self) -> &[OsString] {
        &self.rest
    }
}

pub type MainFn = fn(&mut Ctx, Args) -> ExitCode;

/// คำสั่งหนึ่งตัวในตารางกลาง
#[derive(Debug)]
pub struct Applet {
    pub name: &'static str,
    pub spec: &'static CommandSpec,
    pub main: MainFn,
}

impl Applet {
    pub fn run(&self, ctx: &mut Ctx, args: Args) -> ExitCode {
        (self.main)(ctx, args)
    }
}

/// ตารางคำสั่ง เรียงตามชื่อ
#[derive(Debug)]
pub struct Registry {
    applets: Vec<&'static Applet>,
}

impl Registry {
    /// ชื่อซ้ำหรือชื่อไม่ตรงกับ spec เป็น error
    pub fn new(list: impl IntoIterator<Item = &'static Applet>) -> Result<Self, String> {
        let mut applets: Vec<&'static Applet> = list.into_iter().collect();
        applets.sort_by_key(|a| a.name);
        for w in applets.windows(2) {
            if w[0].name == w[1].name {
                return Err(format!("applet '{}' ลงทะเบียนซ้ำ", w[0].name));
            }
        }
        for a in &applets {
            if a.name != a.spec.name {
                return Err(format!("applet '{}' ใช้ spec ของ '{}'", a.name, a.spec.name));
            }
        }
        Ok(Self { applets })
    }

    pub fn find(&self, name: &str) -> Option<&'static Applet> {
        self.applets
            .binary_search_by_key(&name, |a| a.name)
            .ok()
            .map(|i| self.applets[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static Applet> + '_ {
        self.applets.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.applets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.applets.is_empty()
    }
}

static GLOBAL: OnceLock<Registry> = OnceLock::new();

/// ติดตั้งตารางกลางของ process (เรียกครั้งเดียวตอนเริ่ม) คืน false ถ้าติดตั้งไปแล้ว
pub fn install(reg: Registry) -> bool {
    GLOBAL.set(reg).is_ok()
}

/// ตารางกลางของ process (ใช้โดยคำสั่ง `help`, `which` ฯลฯ)
pub fn global() -> Option<&'static Registry> {
    GLOBAL.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    static SPEC_A: CommandSpec = CommandSpec {
        name: "a",
        ..CommandSpec::BASE
    };
    static SPEC_B: CommandSpec = CommandSpec {
        name: "b",
        ..CommandSpec::BASE
    };

    fn nop(_: &mut Ctx, _: Args) -> ExitCode {
        ExitCode::SUCCESS
    }

    static A: Applet = Applet {
        name: "a",
        spec: &SPEC_A,
        main: nop,
    };
    static B: Applet = Applet {
        name: "b",
        spec: &SPEC_B,
        main: nop,
    };
    static BAD: Applet = Applet {
        name: "a",
        spec: &SPEC_B,
        main: nop,
    };

    #[test]
    fn find_sorts_and_rejects_bad_tables() {
        let r = Registry::new([&B, &A]).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r.iter().map(|a| a.name).collect::<Vec<_>>(), ["a", "b"]);
        assert!(r.find("b").is_some() && r.find("c").is_none());
        assert!(Registry::new([&A, &A]).unwrap_err().contains("ซ้ำ"));
        assert!(Registry::new([&BAD]).unwrap_err().contains("spec"));
    }
}
