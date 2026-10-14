//! `CommandSpec`: นิยามคำสั่งหนึ่งตัวในที่เดียว
//!
//! ใช้ทั้งสำหรับ parse argument และสร้าง `--help` จึงตรงกับ flag จริงเสมอ
//! และใช้ตรวจความครอบคลุมเทียบกับ `flags/<cmd>.toml`

/// flag รับค่าหรือไม่
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// ไม่รับค่า (`--all`)
    None,
    /// ต้องมีค่า (`--width=N` / `--width N` / `-w N` / `-wN`)
    Required,
    /// ค่าเป็นทางเลือก (`--color[=WHEN]` / `-cWHEN` เท่านั้น ไม่กินคำถัดไป)
    Optional,
}

/// วิธีจัดการ operand ระหว่าง option
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseStyle {
    /// สลับตำแหน่งได้ (`ls dir -l`) แบบ GNU; `POSIXLY_CORRECT` จะเปลี่ยนเป็นแบบ Posix
    Gnu,
    /// หยุด parse option ที่ operand แรก (เช่น `env`, `sudo`, `l110`)
    Posix,
    /// คำสั่งแยก argument เอง (`echo`, `true`, `false`); parser กลางไม่ถูกเรียก
    Manual,
}

/// flag หนึ่งตัว ชื่อสั้น/ยาวที่ `id` เดียวกันถือเป็นตัวเดียวกัน
#[derive(Clone, Copy, Debug)]
pub struct OptSpec {
    /// ตัวระบุที่โค้ดคำสั่งใช้ถามผลลัพธ์ (`parsed.flag("all")`)
    pub id: &'static str,
    pub short: Option<char>,
    pub long: Option<&'static str>,
    pub arg: ArgKind,
    /// ชื่อค่าที่แสดงใน help (`NUM`, `FILE`) ว่างเมื่อไม่รับค่า
    pub value_name: &'static str,
    pub help: &'static str,
}

impl OptSpec {
    /// flag ที่ไม่รับค่า
    pub const fn flag(
        id: &'static str,
        short: Option<char>,
        long: Option<&'static str>,
        help: &'static str,
    ) -> Self {
        Self {
            id,
            short,
            long,
            arg: ArgKind::None,
            value_name: "",
            help,
        }
    }

    /// flag ที่ต้องมีค่า
    pub const fn required(
        id: &'static str,
        short: Option<char>,
        long: Option<&'static str>,
        value_name: &'static str,
        help: &'static str,
    ) -> Self {
        Self {
            id,
            short,
            long,
            arg: ArgKind::Required,
            value_name,
            help,
        }
    }

    /// flag ที่ค่าเป็นทางเลือก
    pub const fn optional(
        id: &'static str,
        short: Option<char>,
        long: Option<&'static str>,
        value_name: &'static str,
        help: &'static str,
    ) -> Self {
        Self {
            id,
            short,
            long,
            arg: ArgKind::Optional,
            value_name,
            help,
        }
    }

    /// ชื่อที่ใช้อ้างใน manifest: `--long` ถ้ามี ไม่งั้น `-s`
    pub fn display_name(&self) -> String {
        match (self.long, self.short) {
            (Some(l), _) => format!("--{l}"),
            (None, Some(s)) => format!("-{s}"),
            (None, None) => self.id.to_string(),
        }
    }
}

/// ตัวอย่างการใช้งานที่แสดงใน `--help`
#[derive(Clone, Copy, Debug)]
pub struct Example {
    pub command: &'static str,
    pub description: &'static str,
}

/// นิยามคำสั่งครบชุด
#[derive(Clone, Copy, Debug)]
pub struct CommandSpec {
    pub name: &'static str,
    /// สรุปบรรทัดเดียว (ใช้ในรายการของคำสั่ง `help`)
    pub summary: &'static str,
    /// รูปแบบการเรียก ไม่รวมชื่อคำสั่ง (`"[OPTION]... [FILE]..."`)
    pub usage: &'static [&'static str],
    /// คำอธิบาย (ผู้เขียนจัดบรรทัดเอง ไม่ถูก wrap)
    pub description: &'static str,
    pub options: &'static [OptSpec],
    pub examples: &'static [Example],
    /// ข้อความท้าย help เช่น exit status
    pub footer: &'static str,
    pub parse: ParseStyle,
    /// exit code เมื่อใช้ option ผิด (GNU แต่ละคำสั่งไม่เท่ากัน เช่น `ls`=2, `cat`=1)
    pub usage_status: u8,
}

impl CommandSpec {
    /// ค่าเริ่มต้นสำหรับ `..CommandSpec::BASE`; ฟิลด์ที่ลืมใส่จะถูก `validate` จับ
    pub const BASE: CommandSpec = CommandSpec {
        name: "",
        summary: "",
        usage: &[],
        description: "",
        options: &[],
        examples: &[],
        footer: "",
        parse: ParseStyle::Gnu,
        usage_status: 1,
    };

    /// มี flag ชื่อนี้หรือไม่ (`-n`, `--all`) รวม `--help` / `--version` ที่ระบบเติมให้
    pub fn has_flag(&self, name: &str) -> bool {
        if name == "--help" || name == "--version" {
            return true;
        }
        self.options.iter().any(|o| {
            if let Some(rest) = name.strip_prefix("--") {
                o.long == Some(rest)
            } else if let Some(rest) = name.strip_prefix('-') {
                let mut chars = rest.chars();
                matches!((chars.next(), chars.next()), (Some(c), None) if o.short == Some(c))
            } else {
                false
            }
        })
    }

    /// ตรวจความครบถ้วนของ spec คืนข้อความปัญหาทั้งหมด (ว่าง = ผ่าน)
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let n = self.name;
        if n.is_empty() {
            problems.push("name ว่าง".to_string());
        }
        if self.summary.trim().is_empty() {
            problems.push(format!("{n}: ไม่มี summary"));
        }
        if self.usage.is_empty() || self.usage.iter().any(|u| u.trim().is_empty()) {
            problems.push(format!("{n}: ไม่มี usage"));
        }
        if self.description.trim().is_empty() {
            problems.push(format!("{n}: ไม่มีคำอธิบาย (description)"));
        }
        if self.examples.is_empty() {
            problems.push(format!("{n}: ไม่มีตัวอย่าง (examples)"));
        }
        for e in self.examples {
            if e.command.trim().is_empty() || e.description.trim().is_empty() {
                problems.push(format!("{n}: ตัวอย่างที่ command/description ว่าง"));
            }
        }
        let mut ids = Vec::new();
        let mut shorts = Vec::new();
        let mut longs = Vec::new();
        for o in self.options {
            if o.id.is_empty() {
                problems.push(format!("{n}: option ที่ id ว่าง"));
            }
            if o.short.is_none() && o.long.is_none() {
                problems.push(format!("{n}: option '{}' ไม่มีชื่อสั้น/ยาว", o.id));
            }
            if o.help.trim().is_empty() {
                problems.push(format!("{n}: option '{}' ไม่มีคำอธิบาย", o.id));
            }
            if o.arg != ArgKind::None && o.value_name.is_empty() {
                problems.push(format!("{n}: option '{}' รับค่าแต่ไม่มี value_name", o.id));
            }
            if o.arg == ArgKind::None && !o.value_name.is_empty() {
                problems.push(format!("{n}: option '{}' ไม่รับค่าแต่มี value_name", o.id));
            }
            if o.id == "help" || o.id == "version" || o.long == Some("help") || o.long == Some("version") {
                problems.push(format!("{n}: '{}' ชนกับ --help/--version ที่ระบบเติมให้", o.id));
            }
            // alias ที่ id เดียวกันต้องมี arg kind เหมือนกัน; ชื่อสั้น/ยาวห้ามซ้ำข้าม id
            for prev in self.options.iter().take_while(|p| !std::ptr::eq(*p, o)) {
                if prev.id == o.id && prev.arg != o.arg {
                    problems.push(format!("{n}: id '{}' ซ้ำแต่ชนิดค่าต่างกัน", o.id));
                }
            }
            if !ids.contains(&o.id) {
                ids.push(o.id);
            }
            if let Some(s) = o.short {
                if shorts.contains(&s) && !self.same_id_for_short(s, o.id) {
                    problems.push(format!("{n}: -{s} ซ้ำกันคนละ id"));
                }
                shorts.push(s);
            }
            if let Some(l) = o.long {
                if longs.contains(&l) {
                    problems.push(format!("{n}: --{l} ซ้ำ"));
                }
                longs.push(l);
            }
        }
        problems
    }

    fn same_id_for_short(&self, short: char, id: &str) -> bool {
        self.options
            .iter()
            .filter(|o| o.short == Some(short))
            .all(|o| o.id == id)
    }
}
