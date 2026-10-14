//! parser ของ `flags/<cmd>.toml` (TOML subset: สตริง, comment, `[[flag]]`)
//!
//! ```toml
//! command = "echo"
//! reference = "man echo (GNU coreutils)"
//!
//! [[flag]]
//! name = "-n"
//! status = "done"        # todo | partial | done | n/a
//! note = "..."           # จำเป็นเมื่อ status = "n/a"
//! ```

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Todo,
    Partial,
    Done,
    Na,
}

#[derive(Clone, Debug)]
pub struct Flag {
    pub name: String,
    pub status: Status,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub command: String,
    pub flags: Vec<Flag>,
}

impl Manifest {
    pub fn count(&self, s: Status) -> usize {
        self.flags.iter().filter(|f| f.status == s).count()
    }

    /// ความครอบคลุมเป็นหน่วย 1/100 เปอร์เซ็นต์ (10000 = 100%)
    /// done = 1, partial = 0.5, todo = 0; `n/a` ไม่นับในตัวหาร; ไม่มี flag ที่นับ = 100%
    pub fn coverage_x100(&self) -> u32 {
        let counted = self.flags.len() - self.count(Status::Na);
        if counted == 0 {
            return 10_000;
        }
        let score = 2 * self.count(Status::Done) + self.count(Status::Partial);
        (score * 10_000 / (2 * counted)) as u32
    }
}

pub fn format_pct(x100: u32) -> String {
    format!("{}.{:02}", x100 / 100, x100 % 100)
}

fn unquote(v: &str, line: usize) -> Result<String, String> {
    let v = v.trim();
    let inner = v
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .ok_or_else(|| format!("บรรทัด {line}: ค่าต้องเป็นสตริงในเครื่องหมายคำพูด"))?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                other => {
                    return Err(format!(
                        "บรรทัด {line}: escape ไม่รองรับ: \\{}",
                        other.unwrap_or(' ')
                    ));
                }
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

pub fn parse(text: &str) -> Result<Manifest, String> {
    let mut command = String::new();
    let mut flags: Vec<(Option<String>, Option<String>, String)> = Vec::new();
    let mut in_flag = false;

    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if t == "[[flag]]" {
            flags.push((None, None, String::new()));
            in_flag = true;
            continue;
        }
        if t.starts_with('[') {
            return Err(format!("บรรทัด {line}: section ไม่รองรับ: {t}"));
        }
        let (key, value) = t
            .split_once('=')
            .ok_or_else(|| format!("บรรทัด {line}: ต้องเป็น key = \"value\""))?;
        let key = key.trim();
        // ตัด comment ท้ายบรรทัดเฉพาะเมื่อค่าไม่ใช่สตริงที่มี # อยู่ข้างใน
        let value = strip_trailing_comment(value);
        let value = unquote(&value, line)?;
        if !in_flag {
            match key {
                "command" => command = value,
                "reference" => {}
                _ => return Err(format!("บรรทัด {line}: key ไม่รู้จัก '{key}'")),
            }
        } else {
            let cur = flags.last_mut().expect("in_flag implies an entry");
            match key {
                "name" => cur.0 = Some(value),
                "status" => cur.1 = Some(value),
                "note" => cur.2 = value,
                _ => return Err(format!("บรรทัด {line}: key ไม่รู้จัก '{key}' ใน [[flag]]")),
            }
        }
    }

    if command.is_empty() {
        return Err("ไม่มี command = \"...\"".to_string());
    }
    let mut out = Vec::new();
    for (i, (name, status, note)) in flags.into_iter().enumerate() {
        let name = name.ok_or_else(|| format!("flag #{}: ไม่มี name", i + 1))?;
        if !name.starts_with('-') || name == "-" || name == "--" {
            return Err(format!("flag '{name}': ชื่อต้องเป็น -x หรือ --long"));
        }
        let status = match status.as_deref() {
            Some("todo") => Status::Todo,
            Some("partial") => Status::Partial,
            Some("done") => Status::Done,
            Some("n/a") => Status::Na,
            Some(other) => return Err(format!("flag '{name}': status ไม่ถูกต้อง '{other}'")),
            None => return Err(format!("flag '{name}': ไม่มี status")),
        };
        if status == Status::Na && note.trim().is_empty() {
            return Err(format!("flag '{name}': n/a ต้องระบุเหตุผลใน note"));
        }
        if out.iter().any(|f: &Flag| f.name == name) {
            return Err(format!("flag '{name}': ซ้ำ"));
        }
        out.push(Flag { name, status, note });
    }
    Ok(Manifest {
        command,
        flags: out,
    })
}

fn strip_trailing_comment(v: &str) -> String {
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in v.char_indices() {
        match c {
            '\\' if in_str && !escaped => {
                escaped = true;
                continue;
            }
            '"' if !escaped => in_str = !in_str,
            '#' if !in_str => return v[..i].trim().to_string(),
            _ => {}
        }
        escaped = false;
    }
    v.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# comment
command = "ls"
reference = "man ls"

[[flag]]
name = "-a"
status = "done"

[[flag]]
name = "--author"
status = "n/a"   # trailing comment
note = "ต้องใช้ \"author\" ของ filesystem ที่ WSL ไม่มี # ไม่ใช่ comment"

[[flag]]
name = "-Z"
status = "partial"

[[flag]]
name = "--context"
status = "todo"
"#;

    #[test]
    fn parses_and_scores() {
        let m = parse(SAMPLE).unwrap();
        assert_eq!(m.command, "ls");
        assert_eq!(m.flags.len(), 4);
        assert!(m.flags[1].note.contains("\"author\"") && m.flags[1].note.contains("# ไม่ใช่"));
        // done 1 + partial 0.5 จาก 3 flag ที่นับ (n/a ไม่นับ)
        assert_eq!(m.coverage_x100(), 5000);
        assert_eq!(format_pct(m.coverage_x100()), "50.00");
    }

    #[test]
    fn empty_counts_as_full() {
        let m = parse("command = \"x\"\n").unwrap();
        assert_eq!(m.coverage_x100(), 10_000);
    }

    #[test]
    fn rejects_bad_manifests() {
        assert!(
            parse("command = \"x\"\n[[flag]]\nname = \"-a\"\nstatus = \"n/a\"\n")
                .unwrap_err()
                .contains("เหตุผล")
        );
        assert!(
            parse("command = \"x\"\n[[flag]]\nname = \"a\"\nstatus = \"done\"\n")
                .unwrap_err()
                .contains("ชื่อต้องเป็น")
        );
        assert!(
            parse("command = \"x\"\n[[flag]]\nname = \"-a\"\nstatus = \"maybe\"\n")
                .unwrap_err()
                .contains("status")
        );
        assert!(
            parse("[[flag]]\nname = \"-a\"\nstatus = \"done\"\n")
                .unwrap_err()
                .contains("command")
        );
        assert!(parse("command = \"x\"\n[[flag]]\nname = \"-a\"\nstatus = \"done\"\n[[flag]]\nname = \"-a\"\nstatus = \"done\"\n").unwrap_err().contains("ซ้ำ"));
        assert!(
            parse("command = \"x\"\n[other]\n")
                .unwrap_err()
                .contains("section")
        );
    }
}
