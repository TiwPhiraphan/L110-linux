//! `cargo xtask <task>`: งานดูแลโปรเจกต์
//!
//! M0 มีเฉพาะ `coverage`; `build-rootfs`, `build-repo`, `install` จะเพิ่มใน milestone ถัดไป

mod manifest;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use manifest::{Manifest, Status, format_pct};

const USAGE: &str = "\
Usage: cargo xtask <task>

Tasks:
  coverage [--check | --update [--allow-decrease]]
      Summarize flag coverage from flags/*.toml and cross-check it against the
      real command specs. --check fails if any command is below flags/baseline.txt
      or if a manifest disagrees with its spec. --update rewrites the baseline
      (refused if any command went down, unless --allow-decrease).
  help
      Show this message.
";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the workspace root")
        .to_path_buf()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("coverage") => match coverage(&args[1..]) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("xtask coverage: {e}");
                ExitCode::from(2)
            }
        },
        Some("help") | Some("--help") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("xtask: unknown task '{other}'\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn load_manifests(dir: &Path) -> Result<Vec<(PathBuf, Manifest)>, String> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("อ่าน {} ไม่ได้: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let text = fs::read_to_string(&p).map_err(|e| format!("อ่าน {} ไม่ได้: {e}", p.display()))?;
        let m = manifest::parse(&text).map_err(|e| format!("{}: {e}", p.display()))?;
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        if stem != m.command {
            return Err(format!(
                "{}: ชื่อไฟล์ต้องตรงกับ command (\"{}\")",
                p.display(),
                m.command
            ));
        }
        out.push((p, m));
    }
    Ok(out)
}

fn load_baseline(path: &Path) -> Result<BTreeMap<String, u32>, String> {
    let mut map = BTreeMap::new();
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(map);
    };
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (name, pct) = t
            .split_once(char::is_whitespace)
            .ok_or_else(|| format!("{}:{}: ต้องเป็น 'command percent'", path.display(), i + 1))?;
        let (whole, frac) = pct.trim().split_once('.').unwrap_or((pct.trim(), "00"));
        let whole: u32 = whole
            .parse()
            .map_err(|_| format!("{}:{}: percent ไม่ถูกต้อง", path.display(), i + 1))?;
        let frac: u32 = format!("{frac:0<2}")[..2]
            .parse()
            .map_err(|_| format!("{}:{}: percent ไม่ถูกต้อง", path.display(), i + 1))?;
        map.insert(name.to_string(), whole * 100 + frac);
    }
    Ok(map)
}

/// เทียบ manifest กับ spec จริงของ applet; คืนรายการปัญหา
fn cross_check(m: &Manifest) -> Vec<String> {
    let mut problems = Vec::new();
    let applet = l110::registry().find(&m.command);
    match applet {
        None => {
            for f in &m.flags {
                if matches!(f.status, Status::Done | Status::Partial) {
                    problems.push(format!(
                        "{}: flag {} เป็น {:?} แต่ยังไม่มี applet '{}'",
                        m.command, f.name, f.status, m.command
                    ));
                }
            }
        }
        Some(a) => {
            for f in &m.flags {
                if matches!(f.status, Status::Done | Status::Partial) && !a.spec.has_flag(&f.name) {
                    problems.push(format!(
                        "{}: manifest ระบุ {} เป็น {:?} แต่ spec ไม่มี flag นี้",
                        m.command, f.name, f.status
                    ));
                }
            }
            for o in a.spec.options {
                let in_manifest = [
                    o.long.map(|l| format!("--{l}")),
                    o.short.map(|s| format!("-{s}")),
                ]
                .into_iter()
                .flatten()
                .any(|n| m.flags.iter().any(|f| f.name == n));
                if !in_manifest {
                    problems.push(format!(
                        "{}: spec มี {} แต่ไม่อยู่ใน manifest",
                        m.command,
                        o.display_name()
                    ));
                }
            }
        }
    }
    problems
}

fn coverage(args: &[String]) -> Result<ExitCode, String> {
    let mut check = false;
    let mut update = false;
    let mut allow_decrease = false;
    for a in args {
        match a.as_str() {
            "--check" => check = true,
            "--update" => update = true,
            "--allow-decrease" => allow_decrease = true,
            other => return Err(format!("argument ไม่รู้จัก '{other}'")),
        }
    }
    if check && update {
        return Err("--check และ --update ใช้พร้อมกันไม่ได้".to_string());
    }

    let flags_dir = root().join("flags");
    let baseline_path = flags_dir.join("baseline.txt");
    let manifests = load_manifests(&flags_dir)?;
    let baseline = load_baseline(&baseline_path)?;

    let mut failures: Vec<String> = Vec::new();
    let mut current: BTreeMap<String, u32> = BTreeMap::new();

    println!(
        "{:<12} {:>5} {:>7} {:>5} {:>5}  coverage",
        "command", "done", "partial", "todo", "n/a"
    );
    let (mut sum_done, mut sum_counted) = (0usize, 0usize);
    for (_, m) in &manifests {
        let pct = m.coverage_x100();
        current.insert(m.command.clone(), pct);
        println!(
            "{:<12} {:>5} {:>7} {:>5} {:>5}  {}%",
            m.command,
            m.count(Status::Done),
            m.count(Status::Partial),
            m.count(Status::Todo),
            m.count(Status::Na),
            format_pct(pct)
        );
        for f in m.flags.iter().filter(|f| f.status == Status::Na) {
            println!("             n/a {}: {}", f.name, f.note.trim());
        }
        sum_done += m.count(Status::Done);
        sum_counted += m.flags.len() - m.count(Status::Na);
        failures.extend(cross_check(m));
    }
    println!(
        "\n{} command(s), {sum_done}/{sum_counted} flags done",
        manifests.len()
    );

    let mut dropped: Vec<String> = Vec::new();
    for (name, base) in &baseline {
        match current.get(name) {
            None => dropped.push(format!(
                "{name}: manifest หายไป (baseline {}%)",
                format_pct(*base)
            )),
            Some(now) if now < base => dropped.push(format!(
                "{name}: ความครอบคลุมลดลง {}% -> {}%",
                format_pct(*base),
                format_pct(*now)
            )),
            _ => {}
        }
    }

    if update {
        if !dropped.is_empty() && !allow_decrease {
            return Err(format!(
                "ไม่เขียน baseline เพราะมีคำสั่งที่ลดลง (ใช้ --allow-decrease ถ้าตั้งใจ):\n  {}",
                dropped.join("\n  ")
            ));
        }
        if !failures.is_empty() {
            return Err(format!(
                "manifest ไม่ตรงกับ spec:\n  {}",
                failures.join("\n  ")
            ));
        }
        let mut text = String::from(
            "# ความครอบคลุมขั้นต่ำต่อคำสั่ง (หน่วย %) ปรับด้วย: cargo xtask coverage --update\n",
        );
        for (name, pct) in &current {
            text.push_str(&format!("{name} {}\n", format_pct(*pct)));
        }
        fs::write(&baseline_path, text)
            .map_err(|e| format!("เขียน {} ไม่ได้: {e}", baseline_path.display()))?;
        println!("อัปเดต {}", baseline_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    failures.extend(dropped);
    for name in current.keys().filter(|n| !baseline.contains_key(*n)) {
        if check {
            failures.push(format!(
                "{name}: ยังไม่อยู่ใน flags/baseline.txt (รัน cargo xtask coverage --update)"
            ));
        } else {
            println!("หมายเหตุ: '{name}' ยังไม่อยู่ใน baseline (cargo xtask coverage --update)");
        }
    }
    if check && !failures.is_empty() {
        eprintln!("\ncoverage check ล้มเหลว:");
        for f in &failures {
            eprintln!("  - {f}");
        }
        return Ok(ExitCode::FAILURE);
    }
    if !check && !failures.is_empty() {
        eprintln!("\nคำเตือน:");
        for f in &failures {
            eprintln!("  - {f}");
        }
    }
    Ok(ExitCode::SUCCESS)
}
