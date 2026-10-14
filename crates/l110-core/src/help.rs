//! สร้าง `--help` / `--version` จาก `CommandSpec`

use crate::VERSION;
use crate::spec::{ArgKind, CommandSpec, OptSpec};

const WIDTH: usize = 80;
/// คอลัมน์สูงสุดที่คำอธิบาย option เริ่ม (ถ้าชื่อ option ยาวกว่านี้ ขึ้นบรรทัดใหม่)
const MAX_HELP_COL: usize = 30;

/// ข้อความ `--version`
pub fn render_version(spec: &CommandSpec) -> String {
    format!("{} (L110) {}\n", spec.name, VERSION)
}

/// ข้อความบรรทัด `Try 'x --help' for more information.`
pub fn try_help_line(name: &str) -> String {
    format!("Try '{name} --help' for more information.\n")
}

/// ข้อความ `--help` ทั้งหมด
pub fn render_help(spec: &CommandSpec) -> String {
    let mut s = String::new();

    for (idx, u) in spec.usage.iter().enumerate() {
        if idx == 0 {
            s.push_str(&format!("Usage: {} {}\n", spec.name, u));
        } else {
            s.push_str(&format!("  or:  {} {}\n", spec.name, u));
        }
    }
    if !spec.description.is_empty() {
        s.push_str(spec.description.trim_end());
        s.push('\n');
    }

    let help_opt = OptSpec::flag("help", None, Some("help"), "display this help and exit");
    let version_opt = OptSpec::flag(
        "version",
        None,
        Some("version"),
        "output version information and exit",
    );
    let rows: Vec<(String, &str)> = spec
        .options
        .iter()
        .chain([&help_opt, &version_opt])
        .map(|o| (left_column(o), o.help))
        .collect();

    let widest = rows.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(0);
    let help_col = (widest + 2).min(MAX_HELP_COL);

    s.push('\n');
    for (left, help) in &rows {
        let left_len = left.chars().count();
        let (first_prefix, rest_indent) = if left_len + 2 <= help_col {
            (format!("{left}{}", " ".repeat(help_col - left_len)), help_col)
        } else {
            s.push_str(left);
            s.push('\n');
            (" ".repeat(help_col), help_col)
        };
        let lines = wrap(help, WIDTH.saturating_sub(rest_indent).max(20));
        for (n, line) in lines.iter().enumerate() {
            if n == 0 {
                s.push_str(&first_prefix);
            } else {
                s.push_str(&" ".repeat(rest_indent));
            }
            s.push_str(line);
            s.push('\n');
        }
    }

    if !spec.examples.is_empty() {
        s.push_str("\nExamples:\n");
        for e in spec.examples {
            s.push_str(&format!("  {}\n", e.command));
            for line in wrap(e.description, WIDTH - 6) {
                s.push_str(&format!("      {line}\n"));
            }
        }
    }
    if !spec.footer.is_empty() {
        s.push('\n');
        s.push_str(spec.footer.trim_end());
        s.push('\n');
    }
    s
}

/// ส่วนซ้ายของแถว option เช่น `  -a, --all`, `      --width=COLS`, `  -s[UNIT]`
fn left_column(o: &OptSpec) -> String {
    let mut s = String::from("  ");
    match (o.short, o.long) {
        (Some(c), Some(l)) => {
            s.push_str(&format!("-{c}, --{l}"));
            push_value(&mut s, o, true);
        }
        (None, Some(l)) => {
            s.push_str(&format!("    --{l}"));
            push_value(&mut s, o, true);
        }
        (Some(c), None) => {
            s.push_str(&format!("-{c}"));
            push_value(&mut s, o, false);
        }
        (None, None) => s.push_str(o.id),
    }
    s
}

fn push_value(s: &mut String, o: &OptSpec, has_long: bool) {
    match (o.arg, has_long) {
        (ArgKind::None, _) => {}
        (ArgKind::Required, true) => s.push_str(&format!("={}", o.value_name)),
        (ArgKind::Optional, true) => s.push_str(&format!("[={}]", o.value_name)),
        (ArgKind::Required, false) => s.push_str(&format!(" {}", o.value_name)),
        (ArgKind::Optional, false) => s.push_str(&format!("[{}]", o.value_name)),
    }
}

/// ตัดคำตามความกว้าง (นับเป็นจำนวน char)
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::Example;

    const SPEC: CommandSpec = CommandSpec {
        name: "demo",
        summary: "demo command",
        usage: &["[OPTION]... FILE...", "--other"],
        description: "Do the demo thing.\nSecond line.",
        options: &[
            OptSpec::flag("all", Some('a'), Some("all"), "show everything"),
            OptSpec::required("width", Some('w'), Some("width"), "COLS", "set the width"),
            OptSpec::optional("color", None, Some("color"), "WHEN", "colorize output"),
            OptSpec::optional("size", Some('s'), None, "UNIT", "short only, optional"),
        ],
        examples: &[Example {
            command: "demo -a file",
            description: "show all of file",
        }],
        footer: "Exit status: 0 on success.",
        ..CommandSpec::BASE
    };

    #[test]
    fn help_layout() {
        let h = render_help(&SPEC);
        assert!(h.starts_with("Usage: demo [OPTION]... FILE...\n  or:  demo --other\n"));
        assert!(h.contains("  -a, --all           show everything\n"), "{h}");
        assert!(h.contains("  -w, --width=COLS"), "{h}");
        assert!(h.contains("      --color[=WHEN]"), "{h}");
        assert!(h.contains("  -s[UNIT]"), "{h}");
        assert!(h.contains("      --help          display this help and exit\n"), "{h}");
        assert!(h.contains("      --version       output version information and exit\n"), "{h}");
        assert!(h.contains("\nExamples:\n  demo -a file\n      show all of file\n"));
        assert!(h.ends_with("Exit status: 0 on success.\n"));
    }

    #[test]
    fn version_line() {
        assert_eq!(render_version(&SPEC), format!("demo (L110) {VERSION}\n"));
    }

    #[test]
    fn wrap_respects_width() {
        let lines = wrap("aaa bbb ccc ddd", 7);
        assert_eq!(lines, vec!["aaa bbb", "ccc ddd"]);
    }
}
