//! เทียบ output กับ GNU coreutils จริง; ดู tests/golden/README.md

use std::ffi::OsString;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const L110: &str = env!("CARGO_BIN_EXE_l110");

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

/// แยก argument แบบ shell อย่างง่าย: '...', "...", \x
fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut has_word = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if has_word {
                    words.push(std::mem::take(&mut cur));
                    has_word = false;
                }
            }
            '\'' => {
                has_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => cur.push(ch),
                        None => return Err("single quote ไม่ปิด".into()),
                    }
                }
            }
            '"' => {
                has_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(ch @ ('"' | '\\' | '$' | '`')) => cur.push(ch),
                            Some(ch) => {
                                cur.push('\\');
                                cur.push(ch);
                            }
                            None => return Err("double quote ไม่ปิด".into()),
                        },
                        Some(ch) => cur.push(ch),
                        None => return Err("double quote ไม่ปิด".into()),
                    }
                }
            }
            '\\' => {
                has_word = true;
                match chars.next() {
                    Some(ch) => cur.push(ch),
                    None => return Err("backslash ท้ายบรรทัด".into()),
                }
            }
            c => {
                has_word = true;
                cur.push(c);
            }
        }
    }
    if has_word {
        words.push(cur);
    }
    Ok(words)
}

struct Case {
    status_only: bool,
    env: Vec<(String, String)>,
    name: String,
    args: Vec<String>,
}

fn parse_case(line: &str) -> Result<Case, String> {
    let (status_only, rest) = match line.strip_prefix("~ ") {
        Some(r) => (true, r),
        None => (false, line),
    };
    let mut words = split_words(rest)?.into_iter().peekable();
    let mut env = Vec::new();
    while let Some(w) = words.peek() {
        match w.split_once('=') {
            Some((k, v))
                if !k.is_empty() && k.chars().all(|c| c.is_ascii_uppercase() || c == '_') =>
            {
                env.push((k.to_string(), v.to_string()));
                words.next();
            }
            _ => break,
        }
    }
    let name = words.next().ok_or("ไม่มีชื่อคำสั่ง")?;
    Ok(Case {
        status_only,
        env,
        name,
        args: words.collect(),
    })
}

fn run(mut cmd: Command, case: &Case) -> Output {
    cmd.env_clear()
        .env("LC_ALL", "C")
        .env("PATH", "/usr/bin:/bin");
    for (k, v) in &case.env {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn")
}

fn run_l110(case: &Case) -> Output {
    let mut c = Command::new(L110);
    c.arg0(&case.name).args(&case.args);
    run(c, case)
}

fn run_gnu(path: &str, case: &Case) -> Output {
    let mut c = Command::new(path);
    c.arg0(&case.name).args(&case.args);
    run(c, case)
}

/// หา GNU coreutils; None = ข้าม (หรือ panic เมื่อ L110_REQUIRE_GNU=1)
fn gnu_path(name: &str) -> Option<String> {
    let required = std::env::var_os("L110_REQUIRE_GNU").is_some_and(|v| v == "1");
    for dir in ["/usr/bin", "/bin"] {
        let path = format!("{dir}/{name}");
        if !Path::new(&path).exists() {
            continue;
        }
        let out = Command::new(&path)
            .arg("--version")
            .env("LC_ALL", "C")
            .output();
        let first = out
            .ok()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default();
        if first.contains("GNU coreutils") {
            return Some(path);
        }
        let msg = format!("{path} ไม่ใช่ GNU coreutils (--version: {first:?})");
        assert!(!required, "{msg}");
        eprintln!("ข้าม golden test ของ {name}: {msg}");
        return None;
    }
    assert!(!required, "ไม่พบ GNU {name} ใน /usr/bin หรือ /bin");
    eprintln!("ข้าม golden test ของ {name}: ไม่พบ GNU {name}");
    None
}

fn show(b: &[u8]) -> String {
    format!("{:?}", String::from_utf8_lossy(b))
}

fn run_file(name: &str) {
    let path = cases_dir().join(format!("{name}.cases"));
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let Some(gnu) = gnu_path(name) else {
        return;
    };
    let mut failures = Vec::new();
    let mut total = 0;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        total += 1;
        let case = parse_case(line).unwrap_or_else(|e| panic!("{}:{}: {e}", path.display(), i + 1));
        let want = run_gnu(&gnu, &case);
        let got = run_l110(&case);
        let mut diffs = Vec::new();
        if want.status.code() != got.status.code() {
            diffs.push(format!(
                "exit: GNU={:?} L110={:?}",
                want.status.code(),
                got.status.code()
            ));
        }
        if !case.status_only {
            if want.stdout != got.stdout {
                diffs.push(format!(
                    "stdout: GNU={} L110={}",
                    show(&want.stdout),
                    show(&got.stdout)
                ));
            }
            if want.stderr != got.stderr {
                diffs.push(format!(
                    "stderr: GNU={} L110={}",
                    show(&want.stderr),
                    show(&got.stderr)
                ));
            }
        }
        if !diffs.is_empty() {
            failures.push(format!(
                "{}:{}: {line}\n    {}",
                path.display(),
                i + 1,
                diffs.join("\n    ")
            ));
        }
    }
    assert!(total > 0, "{} ไม่มี case", path.display());
    assert!(
        failures.is_empty(),
        "{} จาก {total} case ไม่ตรง GNU:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn golden_echo() {
    run_file("echo");
}

#[test]
fn golden_true() {
    run_file("true");
}

#[test]
fn golden_false() {
    run_file("false");
}

/// ทุกไฟล์ .cases ต้องมี test ของตัวเอง (กันลืมเพิ่ม test เมื่อเพิ่มไฟล์)
#[test]
fn every_cases_file_has_a_test() {
    let known = ["echo", "true", "false"];
    let mut found: Vec<OsString> = fs::read_dir(cases_dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "cases"))
        .map(|e| e.path().file_stem().unwrap().to_os_string())
        .collect();
    found.sort();
    for f in found {
        assert!(
            known.iter().any(|k| f == *k),
            "{f:?}.cases ยังไม่มี test ใน golden.rs"
        );
    }
}

#[test]
fn splitter_handles_quotes_and_escapes() {
    assert_eq!(
        split_words(r#"echo -e 'a\tb' "c d" e\ f ''"#).unwrap(),
        ["echo", "-e", r"a\tb", "c d", "e f", ""]
    );
    assert_eq!(
        split_words(r#"echo "q\"x\\y\z""#).unwrap(),
        ["echo", r#"q"x\y\z"#]
    );
    assert!(split_words("echo 'open").is_err());
    let c = parse_case("~ POSIXLY_CORRECT=1 echo -n").unwrap();
    assert!(c.status_only && c.env == [("POSIXLY_CORRECT".into(), "1".into())] && c.name == "echo");
}
