//! `true` และ `false`
//!
//! พฤติกรรมตาม GNU coreutils: ไม่ทำอะไรเลย ยกเว้นเมื่อมี argument เดียวที่เป็น
//! `--help` หรือ `--version` ซึ่งจะพิมพ์ข้อความแล้ว exit ด้วย status ของคำสั่งนั้น
//! (`false --help` จึง exit 1)

use l110_core::{Applet, Args, CommandSpec, Ctx, Example, ExitCode, ParseStyle};

pub static TRUE_SPEC: CommandSpec = CommandSpec {
    name: "true",
    summary: "do nothing, successfully",
    usage: &["[ignored command line arguments]", "OPTION"],
    description: "Exit with a status code indicating success.",
    examples: &[Example {
        command: "true && echo ok",
        description: "Always prints ok.",
    }],
    footer: "Any argument other than a lone --help or --version is ignored.",
    parse: ParseStyle::Manual,
    ..CommandSpec::BASE
};

pub static FALSE_SPEC: CommandSpec = CommandSpec {
    name: "false",
    summary: "do nothing, unsuccessfully",
    usage: &["[ignored command line arguments]", "OPTION"],
    description: "Exit with a status code indicating failure.",
    examples: &[Example {
        command: "false || echo failed",
        description: "Always prints failed.",
    }],
    footer: "Any argument other than a lone --help or --version is ignored.",
    parse: ParseStyle::Manual,
    ..CommandSpec::BASE
};

fn run(ctx: &mut Ctx, args: &Args, spec: &CommandSpec, status: u8) -> ExitCode {
    if let [only] = args.rest() {
        if only == "--help" {
            ctx.print_help(spec);
            return ctx.finish(ExitCode(status));
        }
        if only == "--version" {
            ctx.print_version(spec);
            return ctx.finish(ExitCode(status));
        }
    }
    ExitCode(status)
}

fn true_main(ctx: &mut Ctx, args: Args) -> ExitCode {
    run(ctx, &args, &TRUE_SPEC, 0)
}

fn false_main(ctx: &mut Ctx, args: Args) -> ExitCode {
    run(ctx, &args, &FALSE_SPEC, 1)
}

pub static TRUE: Applet = Applet {
    name: "true",
    spec: &TRUE_SPEC,
    main: true_main,
};

pub static FALSE: Applet = Applet {
    name: "false",
    spec: &FALSE_SPEC,
    main: false_main,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn run_applet(a: &Applet, rest: &[&str]) -> (ExitCode, String, String) {
        let (mut ctx, out, err) = Ctx::captured(a.name, &[]);
        let code = a.run(&mut ctx, Args::from_strs(a.name, rest));
        (code, out.text(), err.text())
    }

    #[test]
    fn plain_status() {
        assert_eq!(run_applet(&TRUE, &[]).0, ExitCode(0));
        assert_eq!(run_applet(&FALSE, &[]).0, ExitCode(1));
        // argument อื่นถูกเมินทั้งหมด รวมถึง option ที่ไม่รู้จัก
        let (c, o, e) = run_applet(&TRUE, &["--bogus", "x"]);
        assert_eq!((c, o.as_str(), e.as_str()), (ExitCode(0), "", ""));
        let (c, o, e) = run_applet(&FALSE, &["--help", "x"]);
        assert_eq!((c, o.as_str(), e.as_str()), (ExitCode(1), "", ""));
    }

    #[test]
    fn help_and_version_keep_the_commands_status() {
        let (c, o, _) = run_applet(&TRUE, &["--help"]);
        assert_eq!(c, ExitCode(0));
        assert!(o.starts_with("Usage: true "));
        let (c, o, _) = run_applet(&FALSE, &["--help"]);
        assert_eq!(c, ExitCode(1));
        assert!(o.starts_with("Usage: false "));
        let (c, o, _) = run_applet(&FALSE, &["--version"]);
        assert_eq!(c, ExitCode(1));
        assert!(o.starts_with("false (L110) "));
    }
}
