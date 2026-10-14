//! l110: multi-call binary
//!
//! เรียกเป็น `l110 <applet> [args]` หรือผ่าน symlink ที่ชื่อ applet (`echo` → `l110`)
//! ตารางคำสั่งรวมจากทุก crate ที่นี่ที่เดียว

mod helpcmd;

use std::ffi::OsStr;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::sync::OnceLock;

use l110_core::registry::{self, Registry};
use l110_core::{Applet, Args, CommandSpec, Ctx, Example, ExitCode, OptSpec, ParseStyle};

/// applet ทั้งหมด: crate กลุ่มต่างๆ + คำสั่งที่อยู่ในตัว binary เอง
fn all_applets() -> Vec<&'static Applet> {
    let mut v: Vec<&'static Applet> = Vec::new();
    v.extend_from_slice(l110_sys::APPLETS);
    v.extend_from_slice(helpcmd::APPLETS);
    v
}

/// ตารางคำสั่งกลาง (สร้างครั้งเดียว และติดตั้งเป็น global ของ process ด้วย)
pub fn registry() -> &'static Registry {
    static REG: OnceLock<()> = OnceLock::new();
    REG.get_or_init(|| {
        let reg = Registry::new(all_applets()).expect("applet table must be valid");
        registry::install(reg);
    });
    registry::global().expect("registry installed")
}

/// spec ของตัว `l110` เอง (`l110 --help`)
pub static SPEC: CommandSpec = CommandSpec {
    name: "l110",
    summary: "L110 multi-call binary",
    usage: &["[OPTION]", "APPLET [ARGUMENT]...", "[-s] ..."],
    description: "\
L110 is a userland written in Rust. This single binary provides every command (applet);
the applet is chosen by the name it is invoked as, or by the first argument.

Install a command by linking it to this binary, for example: ln -s l110 /bin/echo",
    options: &[OptSpec::flag(
        "list",
        None,
        Some("list"),
        "list the names of all applets, one per line",
    )],
    examples: &[
        Example {
            command: "l110 echo hello",
            description: "Run the echo applet.",
        },
        Example {
            command: "l110 --list",
            description: "Show every available applet.",
        },
        Example {
            command: "l110 help",
            description: "Show a summary of every command.",
        },
    ],
    footer: "Use 'APPLET --help' for the options of a specific command.",
    // option ของ l110 ต้องมาก่อนชื่อ applet เท่านั้น ส่วนหลังชื่อเป็นของ applet
    parse: ParseStyle::Posix,
    usage_status: 1,
};

/// ชื่อ applet จาก argv[0]: เอา basename และตัด `-` นำหน้า (login shell เช่น `-sh`)
fn applet_name(argv0: &OsStr) -> String {
    let b = argv0.as_bytes();
    let base = b.rsplit(|&c| c == b'/').next().unwrap_or(b);
    let base = base.strip_prefix(b"-").unwrap_or(base);
    String::from_utf8_lossy(base).into_owned()
}

/// จุดเข้าหลัก: เลือกคำสั่งแล้วรัน
pub fn run(ctx: &mut Ctx, args: Args) -> ExitCode {
    let reg = registry();
    let name = applet_name(&args.argv0);

    if name != "l110" {
        return match reg.find(&name) {
            Some(applet) => {
                ctx.prog = name.clone();
                applet.run(ctx, Args::new(name, args.rest))
            }
            None => {
                ctx.prog = name.clone();
                ctx.error("applet not found");
                ExitCode::NOT_FOUND
            }
        };
    }

    ctx.prog = "l110".to_string();
    if args.rest().is_empty() {
        ctx.print_help(&SPEC);
        return ctx.finish(ExitCode::SUCCESS);
    }
    let parsed = match ctx.parse(&SPEC, args.rest()) {
        Ok(p) => p,
        Err(code) => return code,
    };

    if parsed.flag("list") {
        for a in reg.iter() {
            let _ = writeln!(ctx.out, "{}", a.name);
        }
        return ctx.finish(ExitCode::SUCCESS);
    }

    // l110 APPLET [ARGUMENT]...
    let mut operands = parsed.operands.into_iter();
    let first = operands.next().expect("operand follows after option parsing");
    let target = applet_name(&first);
    match reg.find(&target) {
        Some(applet) => {
            ctx.prog = target.clone();
            applet.run(ctx, Args::new(target, operands.collect()))
        }
        None => {
            ctx.prog = "l110".to_string();
            ctx.error(format!("applet not found: {target}"));
            ExitCode::NOT_FOUND
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_main(argv0: &str, rest: &[&str]) -> (ExitCode, String, String) {
        let (mut ctx, out, err) = Ctx::captured(argv0, &[]);
        let code = run(&mut ctx, Args::from_strs(argv0, rest));
        (code, out.text(), err.text())
    }

    #[test]
    fn dispatch_by_argv0_and_by_argument() {
        assert_eq!(run_main("/bin/echo", &["a", "b"]), (ExitCode(0), "a b\n".into(), "".into()));
        assert_eq!(run_main("l110", &["echo", "-n", "x"]).1, "x");
        assert_eq!(run_main("-false", &[]).0, ExitCode(1));
        assert_eq!(run_main("/usr/bin/l110", &["true"]).0, ExitCode(0));
    }

    #[test]
    fn unknown_applet_is_127() {
        let (c, o, e) = run_main("nonesuch", &[]);
        assert_eq!((c, o.as_str(), e.as_str()), (ExitCode(127), "", "nonesuch: applet not found\n"));
        let (c, _, e) = run_main("l110", &["nonesuch"]);
        assert_eq!(c, ExitCode(127));
        assert_eq!(e, "l110: applet not found: nonesuch\n");
    }

    #[test]
    fn own_options_stop_at_the_applet_name() {
        // -n เป็นของ echo ไม่ใช่ของ l110
        assert_eq!(run_main("l110", &["echo", "-n"]).1, "");
        let (c, _, e) = run_main("l110", &["--nope"]);
        assert_eq!(c, ExitCode(1));
        assert!(e.starts_with("l110: unrecognized option '--nope'\nTry 'l110 --help'"));
    }

    #[test]
    fn list_help_version() {
        let (c, o, _) = run_main("l110", &["--list"]);
        assert_eq!(c, ExitCode(0));
        for name in ["echo", "false", "true", "help"] {
            assert!(o.lines().any(|l| l == name), "missing {name} in {o}");
        }
        assert!(run_main("l110", &["--help"]).1.starts_with("Usage: l110 "));
        assert!(run_main("l110", &[]).1.starts_with("Usage: l110 "));
        assert!(run_main("l110", &["--version"]).1.starts_with("l110 (L110) "));
    }

    /// ทุก applet ต้องมี spec/คำอธิบาย/ตัวอย่างครบ และ `--help` ต้องใช้ได้จริง
    #[test]
    fn every_applet_has_complete_spec_and_working_help() {
        let mut problems = SPEC.validate();
        for a in registry().iter() {
            problems.extend(a.spec.validate());
        }
        assert!(problems.is_empty(), "spec ไม่ครบ:\n{}", problems.join("\n"));

        for a in registry().iter() {
            let (mut ctx, out, err) = Ctx::captured(a.name, &[]);
            a.run(&mut ctx, Args::from_strs(a.name, &["--help"]));
            let text = out.text();
            assert!(
                text.starts_with(&format!("Usage: {} ", a.name)),
                "{} --help: {text}",
                a.name
            );
            assert!(text.contains("Examples:"), "{} --help ไม่มีตัวอย่าง", a.name);
            assert!(text.contains("--help"), "{} --help ไม่แสดง --help", a.name);
            assert_eq!(err.text(), "", "{} --help เขียน stderr", a.name);
        }
    }
}
