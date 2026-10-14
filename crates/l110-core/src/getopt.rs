//! parser เทียบเท่า GNU `getopt_long`
//!
//! - flag สั้นรวมกลุ่ม (`-la`), flag ยาว (`--all`), `--opt=val` และ `--opt val`
//! - ย่อ flag ยาวที่ไม่กำกวมได้ (`--rec` → `--recursive`)
//! - optional argument, `--`, การสลับตำแหน่ง option/operand, `POSIXLY_CORRECT`
//! - ข้อความ error ตรงกับ glibc

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

use crate::spec::{ArgKind, CommandSpec, OptSpec, ParseStyle};

/// option ที่ parse ได้หนึ่งตัว (เรียงตามลำดับที่ปรากฏ)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: &'static str,
    pub value: Option<OsString>,
}

/// ผลลัพธ์การ parse
#[derive(Clone, Debug, Default)]
pub struct Parsed {
    items: Vec<Item>,
    pub operands: Vec<OsString>,
}

impl Parsed {
    /// option ทั้งหมดตามลำดับที่ปรากฏ (สำหรับคำสั่งที่ตัวหลังชนะตัวก่อน)
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn flag(&self, id: &str) -> bool {
        self.items.iter().any(|i| i.id == id)
    }

    pub fn count(&self, id: &str) -> usize {
        self.items.iter().filter(|i| i.id == id).count()
    }

    /// ค่าของ option ตัวสุดท้ายที่ระบุ id นี้และมีค่า
    pub fn value(&self, id: &str) -> Option<&OsStr> {
        self.items
            .iter()
            .rev()
            .find(|i| i.id == id)
            .and_then(|i| i.value.as_deref())
    }

    /// ค่าทั้งหมดของ option ที่ระบุซ้ำได้ (`-e a -e b`)
    pub fn values(&self, id: &str) -> Vec<&OsStr> {
        self.items
            .iter()
            .filter(|i| i.id == id)
            .filter_map(|i| i.value.as_deref())
            .collect()
    }
}

/// เหตุที่ parse ไม่ต่อ
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// พบ `--help`
    Help,
    /// พบ `--version`
    Version,
    /// ใช้ผิดวิธี; ข้อความไม่รวมชื่อโปรแกรมนำหน้า (`invalid option -- 'x'`)
    Usage(String),
}

const HELP_OPT: OptSpec = OptSpec::flag("help", None, Some("help"), "");
const VERSION_OPT: OptSpec = OptSpec::flag("version", None, Some("version"), "");

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// parse `args` (ไม่รวม argv[0]) ตาม `spec`; `posixly` คือ `POSIXLY_CORRECT` ถูกตั้งหรือไม่
pub fn parse(spec: &CommandSpec, args: &[OsString], posixly: bool) -> Result<Parsed, ParseError> {
    let stop_at_operand = posixly || spec.parse == ParseStyle::Posix;
    let mut out = Parsed::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_bytes();
        i += 1;
        if arg == b"--" {
            out.operands.extend(args[i..].iter().cloned());
            break;
        }
        if let Some(body) = arg.strip_prefix(b"--") {
            parse_long(spec, body, args, &mut i, &mut out)?;
        } else if arg.len() > 1 && arg[0] == b'-' {
            parse_short_cluster(spec, &arg[1..], args, &mut i, &mut out)?;
        } else {
            out.operands.push(args[i - 1].clone());
            if stop_at_operand {
                out.operands.extend(args[i..].iter().cloned());
                break;
            }
        }
    }
    Ok(out)
}

fn long_candidates<'a>(spec: &'a CommandSpec) -> impl Iterator<Item = (&'static str, OptSpec)> + 'a {
    spec.options
        .iter()
        .filter_map(|o| o.long.map(|l| (l, *o)))
        .chain([("help", HELP_OPT), ("version", VERSION_OPT)])
}

fn parse_long(
    spec: &CommandSpec,
    body: &[u8],
    args: &[OsString],
    i: &mut usize,
    out: &mut Parsed,
) -> Result<(), ParseError> {
    let (name, inline) = match body.iter().position(|&b| b == b'=') {
        Some(p) => (&body[..p], Some(&body[p + 1..])),
        None => (body, None),
    };

    // ตรงทั้งชื่อก่อน; ไม่งั้นลองแบบย่อ
    let mut matches: Vec<(&'static str, OptSpec)> = Vec::new();
    for (l, o) in long_candidates(spec) {
        if l.as_bytes() == name {
            matches = vec![(l, o)];
            break;
        }
        if l.as_bytes().starts_with(name) {
            matches.push((l, o));
        }
    }

    let (full, opt) = match matches.as_slice() {
        [] => {
            return Err(ParseError::Usage(format!(
                "unrecognized option '--{}'",
                lossy(body)
            )));
        }
        [one] => *one,
        many => {
            // ทุกตัวที่ตรงเป็น option เดียวกัน (alias) ไม่ถือว่ากำกวม
            let first = many[0].1;
            let same = many
                .iter()
                .all(|(_, o)| o.id == first.id && o.arg == first.arg);
            if !same {
                let list = many
                    .iter()
                    .map(|(l, _)| format!("'--{l}'"))
                    .collect::<Vec<_>>()
                    .join(" ");
                return Err(ParseError::Usage(format!(
                    "option '--{}' is ambiguous; possibilities: {list}",
                    lossy(body)
                )));
            }
            many[0]
        }
    };

    let value = match (opt.arg, inline) {
        (ArgKind::None, Some(_)) => {
            return Err(ParseError::Usage(format!(
                "option '--{full}' doesn't allow an argument"
            )));
        }
        (ArgKind::None, None) => None,
        (ArgKind::Optional, v) => v.map(|b| OsStr::from_bytes(b).to_os_string()),
        (ArgKind::Required, Some(v)) => Some(OsStr::from_bytes(v).to_os_string()),
        (ArgKind::Required, None) => {
            if *i < args.len() {
                *i += 1;
                Some(args[*i - 1].clone())
            } else {
                return Err(ParseError::Usage(format!(
                    "option '--{full}' requires an argument"
                )));
            }
        }
    };

    match opt.id {
        "help" => return Err(ParseError::Help),
        "version" => return Err(ParseError::Version),
        _ => {}
    }
    out.items.push(Item { id: opt.id, value });
    Ok(())
}

/// ถอด char แรกจาก bytes (UTF-8 ที่ถูกต้อง) คืน (char, ความยาว) ถ้าไม่ถูกต้องคืน None
fn first_char(b: &[u8]) -> Option<(char, usize)> {
    let len = match b.first()? {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return None,
    };
    let s = std::str::from_utf8(b.get(..len)?).ok()?;
    s.chars().next().map(|c| (c, len))
}

fn parse_short_cluster(
    spec: &CommandSpec,
    mut cluster: &[u8],
    args: &[OsString],
    i: &mut usize,
    out: &mut Parsed,
) -> Result<(), ParseError> {
    while !cluster.is_empty() {
        let Some((c, len)) = first_char(cluster) else {
            return Err(ParseError::Usage(format!(
                "invalid option -- '\\{:o}'",
                cluster[0]
            )));
        };
        cluster = &cluster[len..];

        let Some(opt) = spec.options.iter().find(|o| o.short == Some(c)) else {
            return Err(ParseError::Usage(format!("invalid option -- '{c}'")));
        };

        match opt.arg {
            ArgKind::None => out.items.push(Item {
                id: opt.id,
                value: None,
            }),
            ArgKind::Optional => {
                // ค่าต้องติดกับตัว option เท่านั้น
                let value = (!cluster.is_empty()).then(|| OsStr::from_bytes(cluster).to_os_string());
                out.items.push(Item { id: opt.id, value });
                return Ok(());
            }
            ArgKind::Required => {
                let value = if !cluster.is_empty() {
                    OsStr::from_bytes(cluster).to_os_string()
                } else if *i < args.len() {
                    *i += 1;
                    args[*i - 1].clone()
                } else {
                    return Err(ParseError::Usage(format!(
                        "option requires an argument -- '{c}'"
                    )));
                };
                out.items.push(Item {
                    id: opt.id,
                    value: Some(value),
                });
                return Ok(());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTS: &[OptSpec] = &[
        OptSpec::flag("all", Some('a'), Some("all"), "all"),
        OptSpec::flag("long", Some('l'), None, "long"),
        OptSpec::flag("recursive", Some('R'), Some("recursive"), "recursive"),
        OptSpec::flag("reverse", Some('r'), Some("reverse"), "reverse"),
        OptSpec::required("width", Some('w'), Some("width"), "COLS", "width"),
        OptSpec::optional("color", None, Some("color"), "WHEN", "color"),
        OptSpec::optional("size", Some('s'), Some("size"), "UNIT", "size"),
        OptSpec::flag("silent", None, Some("silent"), "silent"),
        OptSpec::flag("quiet", Some('q'), Some("quiet"), "alias of silent"),
        OptSpec::flag("quiet", None, Some("quite"), "alias spelling"),
    ];

    const SPEC: CommandSpec = CommandSpec {
        name: "t",
        options: OPTS,
        ..CommandSpec::BASE
    };

    fn a(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    fn usage(r: Result<Parsed, ParseError>) -> String {
        match r {
            Err(ParseError::Usage(m)) => m,
            other => panic!("expected usage error, got {other:?}"),
        }
    }

    #[test]
    fn short_cluster_and_values() {
        let p = parse(&SPEC, &a(&["-laRw80", "x"]), false).unwrap();
        assert!(p.flag("all") && p.flag("long") && p.flag("recursive"));
        assert_eq!(p.value("width").unwrap(), "80");
        assert_eq!(p.operands, a(&["x"]));
        let p = parse(&SPEC, &a(&["-w", "7"]), false).unwrap();
        assert_eq!(p.value("width").unwrap(), "7");
    }

    #[test]
    fn long_forms_and_abbreviation() {
        let p = parse(&SPEC, &a(&["--rec", "--width=5", "--wid", "9"]), false).unwrap();
        assert!(p.flag("recursive"));
        assert_eq!(p.values("width"), vec!["5", "9"]);
        // ตรงทั้งชื่อชนะการย่อ
        let p = parse(&SPEC, &a(&["--re"]), false);
        assert!(usage(p).contains("ambiguous; possibilities: '--recursive' '--reverse'"));
    }

    #[test]
    fn alias_with_same_id_is_not_ambiguous() {
        let p = parse(&SPEC, &a(&["--qui"]), false).unwrap();
        assert!(p.flag("quiet"));
    }

    #[test]
    fn optional_argument_rules() {
        let p = parse(&SPEC, &a(&["--color", "x"]), false).unwrap();
        assert_eq!(p.value("color"), None);
        assert_eq!(p.operands, a(&["x"]));
        let p = parse(&SPEC, &a(&["--color=always", "-sK"]), false).unwrap();
        assert_eq!(p.value("color").unwrap(), "always");
        assert_eq!(p.value("size").unwrap(), "K");
    }

    #[test]
    fn permutation_double_dash_and_posix() {
        let p = parse(&SPEC, &a(&["f1", "-a", "f2", "--", "-l"]), false).unwrap();
        assert!(p.flag("all") && !p.flag("long"));
        assert_eq!(p.operands, a(&["f1", "f2", "-l"]));
        let p = parse(&SPEC, &a(&["f1", "-a"]), true).unwrap();
        assert!(!p.flag("all"));
        assert_eq!(p.operands, a(&["f1", "-a"]));
        // "-" เป็น operand
        let p = parse(&SPEC, &a(&["-", "-a"]), false).unwrap();
        assert_eq!(p.operands, a(&["-"]));
        assert!(p.flag("all"));
    }

    #[test]
    fn error_messages_match_glibc() {
        assert_eq!(usage(parse(&SPEC, &a(&["-z"]), false)), "invalid option -- 'z'");
        assert_eq!(
            usage(parse(&SPEC, &a(&["--nope=1"]), false)),
            "unrecognized option '--nope=1'"
        );
        assert_eq!(
            usage(parse(&SPEC, &a(&["--all=1"]), false)),
            "option '--all' doesn't allow an argument"
        );
        assert_eq!(
            usage(parse(&SPEC, &a(&["--wid"]), false)),
            "option '--width' requires an argument"
        );
        assert_eq!(
            usage(parse(&SPEC, &a(&["-w"]), false)),
            "option requires an argument -- 'w'"
        );
    }

    #[test]
    fn help_and_version() {
        assert_eq!(parse(&SPEC, &a(&["-a", "--help"]), false).unwrap_err(), ParseError::Help);
        assert_eq!(parse(&SPEC, &a(&["--vers"]), false).unwrap_err(), ParseError::Version);
        assert!(usage(parse(&SPEC, &a(&["--help=x"]), false)).contains("doesn't allow an argument"));
    }
}
