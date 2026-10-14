//! คำสั่ง `help`: รายการคำสั่งทั้งหมด หรือ `help <cmd>...` แสดง help ของคำสั่งนั้น

use std::io::Write;

use l110_core::help::render_help;
use l110_core::registry::global;
use l110_core::{Applet, Args, CommandSpec, Ctx, Example, ExitCode, ParseStyle};

pub static SPEC: CommandSpec = CommandSpec {
    name: "help",
    summary: "show help for L110 commands",
    usage: &["[COMMAND]..."],
    description: "\
Without arguments, list every available command with a one-line summary.
With one or more COMMANDs, show the full help of each (same as 'COMMAND --help').",
    examples: &[
        Example {
            command: "help",
            description: "List all commands.",
        },
        Example {
            command: "help echo",
            description: "Show the help of echo.",
        },
    ],
    footer: "Exit status: 0 if every COMMAND was found, 1 otherwise.",
    parse: ParseStyle::Gnu,
    ..CommandSpec::BASE
};

fn main(ctx: &mut Ctx, args: Args) -> ExitCode {
    let parsed = match ctx.parse(&SPEC, args.rest()) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let Some(reg) = global() else {
        ctx.error("command table is not available");
        return ExitCode::FAILURE;
    };

    if parsed.operands.is_empty() {
        let width = reg.iter().map(|a| a.name.len()).max().unwrap_or(0);
        let _ = writeln!(ctx.out, "L110 commands (use 'help COMMAND' for details):\n");
        for a in reg.iter() {
            let _ = writeln!(ctx.out, "  {:<width$}  {}", a.name, a.spec.summary);
        }
        return ctx.finish(ExitCode::SUCCESS);
    }

    let mut status = ExitCode::SUCCESS;
    for (n, op) in parsed.operands.iter().enumerate() {
        let name = op.to_string_lossy();
        match reg.find(&name) {
            Some(a) => {
                if n > 0 {
                    let _ = writeln!(ctx.out);
                }
                let _ = ctx.out.write_all(render_help(a.spec).as_bytes());
            }
            None => {
                ctx.error(format!("no help topics match '{name}'"));
                status = ExitCode::FAILURE;
            }
        }
    }
    ctx.finish(status)
}

pub static HELP: Applet = Applet {
    name: "help",
    spec: &SPEC,
    main,
};

pub static APPLETS: &[&Applet] = &[&HELP];

#[cfg(test)]
mod tests {
    use super::*;

    fn run_help(rest: &[&str]) -> (ExitCode, String, String) {
        crate::registry();
        let (mut ctx, out, err) = Ctx::captured("help", &[]);
        let code = HELP.run(&mut ctx, Args::from_strs("help", rest));
        (code, out.text(), err.text())
    }

    #[test]
    fn lists_commands_with_summaries() {
        let (c, o, _) = run_help(&[]);
        assert_eq!(c, ExitCode::SUCCESS);
        assert!(o.contains("  echo   display a line of text\n"), "{o}");
        assert!(o.contains("  true   do nothing, successfully\n"), "{o}");
    }

    #[test]
    fn shows_help_for_named_commands_and_reports_unknown() {
        let (c, o, e) = run_help(&["echo", "nonesuch", "false"]);
        assert_eq!(c, ExitCode::FAILURE);
        assert!(o.contains("Usage: echo ") && o.contains("Usage: false "));
        assert_eq!(e, "help: no help topics match 'nonesuch'\n");
    }
}
