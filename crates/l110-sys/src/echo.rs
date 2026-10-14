//! `echo`
//!
//! ทำตาม `echo.c` ของ GNU coreutils:
//! - option นำหน้า `-n -e -E` รวมกลุ่มได้ (`-ne`) แต่ถ้า argument ที่ขึ้นต้นด้วย `-` มีตัวอื่นปนอยู่
//!   (`-nx`) หรือเป็น `-` เฉยๆ จะถือเป็นข้อความธรรมดา และหยุดหา option ตั้งแต่ตรงนั้น
//! - `--help` / `--version` ใช้ได้เฉพาะเมื่อเป็น argument เดียว และไม่ได้ตั้ง `POSIXLY_CORRECT`
//! - `POSIXLY_CORRECT`: ไม่รับ option ยกเว้นเมื่อ argument แรกคือ `-n` พอดี และตีความ backslash เสมอ
//! - argument เป็น byte ดิบ (ไม่บังคับ UTF-8)

use std::os::unix::ffi::OsStrExt;

use l110_core::{Applet, Args, CommandSpec, Ctx, Example, ExitCode, OptSpec, ParseStyle};

pub static SPEC: CommandSpec = CommandSpec {
    name: "echo",
    summary: "display a line of text",
    usage: &["[SHORT-OPTION]... [STRING]...", "LONG-OPTION"],
    description: "\
Echo the STRING(s) to standard output.

If -e is in effect, the following sequences are recognized:

  \\\\      backslash
  \\a      alert (BEL)
  \\b      backspace
  \\c      produce no further output
  \\e      escape
  \\f      form feed
  \\n      new line
  \\r      carriage return
  \\t      horizontal tab
  \\v      vertical tab
  \\0NNN   byte with octal value NNN (1 to 3 digits)
  \\xHH    byte with hexadecimal value HH (1 to 2 digits)",
    options: &[
        OptSpec::flag("n", Some('n'), None, "do not output the trailing newline"),
        OptSpec::flag(
            "e",
            Some('e'),
            None,
            "enable interpretation of backslash escapes",
        ),
        OptSpec::flag(
            "E",
            Some('E'),
            None,
            "disable interpretation of backslash escapes (default)",
        ),
    ],
    examples: &[
        Example {
            command: "echo hello world",
            description: "Print the words separated by a single space.",
        },
        Example {
            command: "echo -n prompt:",
            description: "Print without the trailing newline.",
        },
        Example {
            command: "echo -e 'a\\tb\\nc'",
            description: "Print a tab and a newline using escapes.",
        },
    ],
    footer: "\
Options are only recognised at the start of the argument list. The shell may have its own\n\
builtin echo with different behavior; use /bin/echo (or env echo) to get this one.",
    parse: ParseStyle::Manual,
    ..CommandSpec::BASE
};

fn main(ctx: &mut Ctx, args: Args) -> ExitCode {
    let argv = args.rest();
    let posixly = ctx.posixly_correct();

    if !posixly {
        if let [only] = argv {
            if only == "--help" {
                ctx.print_help(&SPEC);
                return ctx.finish(ExitCode::SUCCESS);
            }
            if only == "--version" {
                ctx.print_version(&SPEC);
                return ctx.finish(ExitCode::SUCCESS);
            }
        }
    }

    let mut newline = true;
    let mut escapes = false;
    let mut start = 0;

    let allow_options = !posixly || argv.first().is_some_and(|a| a == "-n");
    if allow_options {
        while let Some(arg) = argv.get(start) {
            let b = arg.as_bytes();
            let Some(flags) = b.strip_prefix(b"-") else {
                break;
            };
            if flags.is_empty() || !flags.iter().all(|c| matches!(c, b'n' | b'e' | b'E')) {
                break;
            }
            for c in flags {
                match c {
                    b'n' => newline = false,
                    b'e' => escapes = true,
                    _ => escapes = false,
                }
            }
            start += 1;
        }
    }

    let interpret = escapes || posixly;
    let mut out: Vec<u8> = Vec::new();
    let mut stopped = false;
    for (n, arg) in argv[start..].iter().enumerate() {
        if n > 0 {
            out.push(b' ');
        }
        if interpret {
            if unescape(arg.as_bytes(), &mut out) {
                stopped = true;
                break;
            }
        } else {
            out.extend_from_slice(arg.as_bytes());
        }
    }
    if newline && !stopped {
        out.push(b'\n');
    }

    if let Err(e) = ctx.out.write_all(&out) {
        ctx.error(format!("write error: {}", l110_core::strerror(&e)));
        return ExitCode::FAILURE;
    }
    ctx.finish(ExitCode::SUCCESS)
}

/// ตีความ backslash ของ `s` ต่อท้าย `out`; คืน true เมื่อเจอ `\c` (หยุดผลลัพธ์ทั้งหมด)
fn unescape(s: &[u8], out: &mut Vec<u8>) -> bool {
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        i += 1;
        if c != b'\\' || i >= s.len() {
            out.push(c);
            continue;
        }
        let e = s[i];
        i += 1;
        match e {
            b'a' => out.push(0x07),
            b'b' => out.push(0x08),
            b'c' => return true,
            b'e' => out.push(0x1b),
            b'f' => out.push(0x0c),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'v' => out.push(0x0b),
            b'\\' => out.push(b'\\'),
            b'x' => match s.get(i).and_then(|&h| hex(h)) {
                Some(hi) => {
                    i += 1;
                    let mut v = hi;
                    if let Some(lo) = s.get(i).and_then(|&h| hex(h)) {
                        i += 1;
                        v = v * 16 + lo;
                    }
                    out.push(v);
                }
                // `\x` ที่ไม่มี hex digit ตามหลัง: พิมพ์ backslash แล้วตามด้วย x
                None => out.extend_from_slice(b"\\x"),
            },
            b'0' => {
                // `\0` ตามด้วย octal ได้อีกสูงสุด 3 หลัก
                let mut v: u32 = 0;
                let mut taken = 0;
                while taken < 3 && s.get(i).is_some_and(|d| (b'0'..=b'7').contains(d)) {
                    v = v * 8 + u32::from(s[i] - b'0');
                    i += 1;
                    taken += 1;
                }
                out.push(v as u8);
            }
            b'1'..=b'7' => {
                // `\NNN` ไม่มี 0 นำ: รวมตัวนี้แล้วสูงสุด 3 หลัก
                let mut v = u32::from(e - b'0');
                let mut taken = 1;
                while taken < 3 && s.get(i).is_some_and(|d| (b'0'..=b'7').contains(d)) {
                    v = v * 8 + u32::from(s[i] - b'0');
                    i += 1;
                    taken += 1;
                }
                out.push(v as u8);
            }
            other => {
                out.push(b'\\');
                out.push(other);
            }
        }
    }
    false
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub static APPLET: Applet = Applet {
    name: "echo",
    spec: &SPEC,
    main,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn echo(env: &[(&str, &str)], rest: &[&str]) -> (ExitCode, Vec<u8>) {
        let (mut ctx, out, _err) = Ctx::captured("echo", env);
        let code = APPLET.run(&mut ctx, Args::from_strs("echo", rest));
        (code, out.bytes())
    }

    fn text(rest: &[&str]) -> String {
        String::from_utf8_lossy(&echo(&[], rest).1).into_owned()
    }

    #[test]
    fn basic() {
        assert_eq!(text(&[]), "\n");
        assert_eq!(text(&["a", "b  c", ""]), "a b  c \n");
        assert_eq!(text(&["-n", "x"]), "x");
        assert_eq!(text(&["-n"]), "");
    }

    #[test]
    fn option_clusters_and_literal_dash_args() {
        assert_eq!(text(&["-ne", "a\\nb"]), "a\nb");
        assert_eq!(text(&["-nx", "a"]), "-nx a\n");
        assert_eq!(text(&["-", "a"]), "- a\n");
        assert_eq!(text(&["--", "a"]), "-- a\n");
        // -e แล้ว -E ตัวหลังชนะ
        assert_eq!(text(&["-e", "-E", "a\\tb"]), "a\\tb\n");
        // หยุดหา option ที่ argument แรกที่ไม่ใช่ option
        assert_eq!(text(&["x", "-n"]), "x -n\n");
        assert_eq!(text(&["-n", "-n", "-e", "z"]), "z");
    }

    #[test]
    fn escapes() {
        assert_eq!(text(&["-e", "a\\\\b\\tc"]), "a\\b\tc\n");
        assert_eq!(text(&["-e", "x\\cy", "z"]), "x");
        assert_eq!(text(&["-e", "\\x41\\x4a\\xZ"]), "AJ\\xZ\n");
        assert_eq!(text(&["-e", "\\0101\\101\\0"]), "AA\0\n");
        assert_eq!(text(&["-e", "\\q\\"]), "\\q\\\n");
        assert_eq!(echo(&[], &["-e", "\\xff"]).1, vec![0xff, b'\n']);
        // \c ตัดทั้งบรรทัด รวมถึง newline และ argument ถัดไป
        assert_eq!(text(&["-e", "a\\c", "b"]), "a");
    }

    #[test]
    fn non_utf8_arguments_pass_through() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let (mut ctx, out, _) = Ctx::captured("echo", &[]);
        let arg = OsString::from_vec(vec![0xff, 0xfe]);
        let code = APPLET.run(&mut ctx, Args::new("echo", vec![arg]));
        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(out.bytes(), vec![0xff, 0xfe, b'\n']);
    }

    #[test]
    fn help_version_only_when_lone_argument() {
        assert!(text(&["--help"]).starts_with("Usage: echo "));
        assert!(text(&["--version"]).starts_with("echo (L110) "));
        assert_eq!(text(&["--help", "x"]), "--help x\n");
        assert_eq!(text(&["x", "--version"]), "x --version\n");
    }

    #[test]
    fn posixly_correct_mode() {
        let p = [("POSIXLY_CORRECT", "1")];
        let t = |rest: &[&str]| String::from_utf8_lossy(&echo(&p, rest).1).into_owned();
        // ไม่รับ option ยกเว้น -n นำหน้า
        assert_eq!(t(&["-e", "a"]), "-e a\n");
        assert_eq!(t(&["--help"]), "--help\n");
        assert_eq!(t(&["-n", "a"]), "a");
        // ตีความ backslash เสมอ
        assert_eq!(t(&["a\\tb"]), "a\tb\n");
    }

    #[test]
    fn write_error_reports_and_fails() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from_raw_os_error(28))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let err = l110_core::Capture::default();
        let mut ctx = Ctx::new(
            "echo",
            Box::new(Broken),
            Box::new(err.clone()),
            Box::new(std::io::empty()),
            Default::default(),
        );
        let code = APPLET.run(&mut ctx, Args::from_strs("echo", &["x"]));
        assert_eq!(code, ExitCode::FAILURE);
        assert_eq!(err.text(), "echo: write error: No space left on device\n");
    }
}
