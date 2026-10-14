# L110-linux — แผนโปรเจกต์

> userland ของ Linux ที่เขียนเองด้วย Rust ทั้งหมด แพ็กเป็น rootfs สำหรับ `wsl --import` (ใช้ kernel ของ WSL)

## 1. การตัดสินใจที่ล็อกแล้ว

| หัวข้อ | ข้อตกลง |
|---|---|
| ภาษา | Rust (stable) |
| แนวทาง | A: userland จริง + rootfs สำหรับ WSL (ไม่เขียน kernel) |
| เป้าหมายรัน | WSL2 เท่านั้น (x86_64 เป็นหลัก, aarch64 เป็นตัวเลือก) |
| Prompt | `L110:~/%s $` โดย `%s` = path ปัจจุบัน (ดู §4) |
| ความครอบคลุม | ทำครบทุกคำสั่งและทุก flag ตาม man page ของ GNU (ดู §6) |
| sudo | มีในตัว (setuid, อ่าน `/etc/sudoers`) |
| apt | มีในตัว (รูปแบบแพ็กเกจและ repo ของ L110 เอง) |
| help | ทุกคำสั่งมี `--help` และมีคำสั่ง `help` |
| sh | `/bin/sh` = shell ของ L110 เอง (แนว POSIX sh) |
| bash | ไม่เขียนเอง ติดตั้งผ่าน `apt install bash` จาก repo ของ L110 (ดู §8.1) |

## 2. โครงสร้าง Cargo workspace

```
L110-linux/
├─ Cargo.toml                 # workspace
├─ crates/
│  ├─ l110-core/              # applet registry, arg parser (getopt_long), help generator, error type, fs/syscall helpers
│  ├─ l110-shell/             # lexer, parser, expansion, executor, line editor, history, builtins
│  ├─ l110-fs/                # ls cp mv rm mkdir ln stat chmod chown touch du df find ...
│  ├─ l110-text/              # cat head tail sort uniq cut tr sed grep wc diff ...
│  ├─ l110-sys/               # ps kill uname date env id whoami mount ...
│  ├─ l110-archive/           # tar gzip gunzip (deflate เขียนเอง)
│  ├─ l110-users/             # parser passwd/group/shadow, sha512-crypt, su, useradd ...
│  ├─ l110-sudo/              # sudo + sudoers parser
│  ├─ l110-apt/               # apt, dpkg-lite, resolver, repo client
│  └─ l110/                   # main: multi-call binary (dispatch ตาม argv[0])
├─ xtask/                     # build-rootfs, build-repo, coverage, install
├─ tests/
│  ├─ golden/                 # เทียบ output กับ GNU coreutils
│  └─ integration/            # ทดสอบใน rootfs จริง
├─ flags/                     # manifest ของ flag ต่อคำสั่ง (*.toml) ใช้ติดตามความครอบคลุม
├─ scripts/install.ps1        # ติดตั้งเข้า WSL ด้วยคำสั่งเดียว
└─ docs/
```

ผลลัพธ์ build คือ binary เดียว `l110` (static, target `x86_64-unknown-linux-musl`) และทุกคำสั่งเป็น symlink ชี้มาที่มัน

### ข้อกำหนดเรื่อง dependency
- ยอมให้ใช้ crate ระดับล่าง เช่น `libc` / `rustix` สำหรับ syscall เท่านั้น
- คำสั่ง, shell, parser, deflate, sha, crypt เขียนเองทั้งหมด (ถ้าจะใช้ crate เพิ่ม ให้เสนอก่อน)

## 3. l110-core

### 3.1 Applet
```rust
pub struct Applet {
    pub name: &'static str,
    pub spec: &'static CommandSpec,   // flag, argument, ตัวอย่าง, คำอธิบาย
    pub main: fn(&mut Ctx, Args) -> ExitCode,
}
```
ทุกคำสั่งลงทะเบียนในตารางกลางตอน compile

### 3.2 Arg parser (พฤติกรรมเทียบเท่า GNU getopt_long)
- flag สั้นแบบรวมกลุ่ม (`-la`), flag ยาว (`--all`), ค่าแบบ `--opt=val` และ `--opt val`
- รองรับการย่อ flag ยาวที่ไม่กำกวม (`--rec` → `--recursive`)
- optional argument, `--`, การสลับตำแหน่ง option/operand (permutation), `POSIXLY_CORRECT`
- ข้อความ error ตรงกับ GNU (`invalid option -- 'x'`, `Try 'ls --help' for more information.`)
- exit code ตาม GNU (1 = error ทั่วไป, 2 = ใช้ผิดวิธี ฯลฯ)

### 3.3 Help generator
- `cmd --help` สร้างจาก `CommandSpec` ที่เดียวกับที่ใช้ parse จึงตรงกับ flag จริงเสมอ
- `cmd --version`
- คำสั่ง `help` (ทั้งแบบรายการคำสั่ง และ `help <cmd>`) และ `man <cmd>` แบบย่อ
- test ที่ล้มทันทีถ้ามี applet ไหนไม่มี spec/คำอธิบาย/ตัวอย่าง

## 4. Shell

### Prompt
รูปแบบ `L110:~/%s $` โดยแทน `%s` ด้วย path ปัจจุบันดังนี้

| ตำแหน่ง | Prompt |
|---|---|
| `$HOME` | `L110:~/ $` |
| `$HOME/proj/src` | `L110:~/proj/src $` |
| นอก home เช่น `/etc` | `L110:/etc $` |

ใช้ `#` แทน `$` เมื่อเป็น root และตั้งค่ารูปแบบผ่าน `PS1` ได้ ถ้ากำหนดกฎของ `%s` ต่างจากนี้ ให้แก้ที่ตารางนี้

### ความสามารถ
- Lexing/parsing: quoting (`'` `"` `\`), comment, heredoc, `;` `&` `&&` `||` `|` `!`, subshell `( )`, group `{ }`
- Expansion: ตัวแปร `$VAR ${VAR:-x}` ฯลฯ, `$(cmd)`, `$((expr))`, tilde, brace, glob (`* ? [..]`)
- Redirect: `< > >> 2> &> 2>&1 <<`
- Control flow: `if/elif/else`, `while`, `until`, `for`, `case`, function
- Job control: `&`, `jobs`, `fg`, `bg`, `Ctrl-C/Z`, process group
- Line editor เขียนเอง: ลูกศร, history (`~/.l110_history`), `Ctrl-R`, tab-complete
- Builtins: `cd pwd echo export unset set alias unalias type source . exit read test [ ] true false exec command wait times umask ulimit trap help history`
- Script: `l110 script.sh` และ shebang

## 5. รายการคำสั่ง (ขอบเขตทั้งหมด)

**ไฟล์และไดเรกทอรี:** `ls cp mv rm mkdir rmdir ln touch stat chmod chown chgrp readlink realpath basename dirname mktemp install dd sync truncate shred du df find xargs tree file`

**ข้อความ:** `cat tac head tail wc sort uniq cut paste join tr sed grep egrep fgrep nl od hexdump fold fmt expand unexpand comm diff cmp tee split csplit rev strings less more`

**ระบบ/ข้อมูล:** `echo printf pwd env printenv uname hostname whoami id groups date sleep uptime free ps top kill killall pkill pgrep nice renice nohup time timeout dmesg lsblk mount umount clear reset tty stty seq yes true false expr test factor`

**ผู้ใช้:** `su useradd userdel usermod groupadd groupdel groupmod passwd chpasswd chsh login who w last`

**บีบอัด/archive:** `tar gzip gunzip zcat bzip2? xz?` (bzip2/xz เป็นตัวเลือกท้ายแผน)

**ระบบแพ็กเกจและสิทธิ์:** `sudo visudo apt apt-get apt-cache dpkg-lite`

**อื่น ๆ:** `which whereis alias type watch cal bc? vi/nano-lite?` (ตัวที่มี `?` เป็นตัวเลือก ตัดสินใจกันตอนใกล้ถึง)

## 6. นิยามคำว่า "ครบ" และวิธีวัด

"ทุก flag" ต้องวัดได้ จึงกำหนดแบบนี้

1. **เป้าหมายอ้างอิง:** GNU coreutils / util-linux / procps-ng / findutils / grep / sed / tar เวอร์ชันที่ Ubuntu LTS ล่าสุดใช้อยู่ (ล็อกเวอร์ชันไว้ใน `docs/reference-versions.md`)
2. **Manifest:** `flags/<cmd>.toml` ลิสต์ทุก flag จาก man page ของคำสั่งนั้นพร้อมสถานะ `todo | partial | done | n/a` (`n/a` ต้องระบุเหตุผล เช่น SELinux หรือ feature ที่ WSL ไม่มี)
3. **Coverage report:** `cargo xtask coverage` สรุปเปอร์เซ็นต์ต่อคำสั่ง และ CI ล้มถ้าความครอบคลุมลดลง
4. **Golden test:** แต่ละ flag ที่ `done` ต้องมี test เทียบ output กับ GNU จริง (รันเทียบกันใน CI บน Ubuntu)
5. **นิยามเสร็จ:** ทุก flag เป็น `done` หรือ `n/a` ที่มีเหตุผล

หมายเหตุที่ควรรู้: นี่เป็นงานปริมาณมาก (ระดับหลายหมื่นบรรทัด) การทำ "ให้ครบ" จึงควรทำเป็นรอบ ๆ ตามลำดับด้านล่าง แต่ไม่ลดเป้าหมาย คือไม่ปิดโปรเจกต์จนกว่า manifest จะครบ ลำดับที่ใช้งานได้ก่อนช่วยให้ทดสอบกับ WSL จริงได้ตั้งแต่ต้น

## 7. ระบบผู้ใช้และ sudo

- ไฟล์ `/etc/passwd`, `/etc/group`, `/etc/shadow` (สิทธิ์ 0640 root:shadow), `/etc/sudoers`, `/etc/sudoers.d/`
- hash รหัสผ่านแบบ SHA-512 crypt (`$6$`) เขียนเอง, ไม่มี plaintext
- `sudo`:
  - binary setuid root (ทำงานได้เฉพาะบน ext4 ของ rootfs ไม่ใช่ `/mnt/c`)
  - parser ของ sudoers: user/group/`ALL`, `NOPASSWD`, `Defaults` พื้นฐาน, alias, `!` negation
  - `sudo -u -g -i -s -E -k -K -l -v -n -S -H -b --` ตามที่ GNU/sudo มี
  - ล้าง env ตามนโยบาย `env_reset`, ตั้ง `secure_path`, จำกัดจำนวนครั้งที่ใส่รหัสผิด, timestamp cache แยกต่อ tty, log ที่ `/var/log/auth.log`
  - `visudo` ตรวจ syntax ก่อนบันทึก
- `su`, `login`, และการตั้ง default user ผ่าน `/etc/wsl.conf` (`[user] default=`)
- ความเสี่ยงสูง: ส่วนนี้ต้อง review เรื่องความปลอดภัยแยก (race condition, PATH/env injection, symlink attack)

## 8. apt

### รูปแบบแพ็กเกจ `.l110pkg`
```
name-version-arch.l110pkg   (tar.gz)
├─ control        # Package, Version, Architecture, Depends, Conflicts, Provides, Description
├─ files/         # ไฟล์ที่ติดตั้งลงรูท
└─ scripts/       # preinst postinst prerm postrm (ถ้ามี)
```

### Repo
- `index.json` (รายการแพ็กเกจ, เวอร์ชัน, sha256, ขนาด, depends) + ไฟล์แพ็กเกจ
- โฮสต์บน GitHub Pages/Releases ของรีโปนี้ สร้างด้วย `cargo xtask build-repo`
- ลายเซ็น index (ed25519) พร้อม keyring ใน `/etc/apt/trusted.d/` (ทำใน milestone ความปลอดภัย)

### คำสั่ง
`apt update | upgrade | full-upgrade | install | reinstall | remove | purge | autoremove | search | show | list | policy | depends | rdepends | download | clean | edit-sources` และ `apt-get`, `apt-cache` ที่ alias ไปยัง logic เดียวกัน

### Resolver
- จัดการ `Depends`, `Conflicts`, `Provides`, constraint ของเวอร์ชัน (`>= = <<`), เปรียบเทียบเวอร์ชันแบบ Debian
- ตรวจ dependency วนและแสดงแผนก่อนติดตั้ง (`The following NEW packages will be installed...`) พร้อมถามยืนยัน (`-y` ข้าม)
- ฐานข้อมูลที่ `/var/lib/apt/` (รายการติดตั้ง, รายการไฟล์ต่อแพ็กเกจ), cache ที่ `/var/cache/apt/`, lock file, รองรับ rollback เมื่อติดตั้งล้มกลางทาง

### แพ็กเกจเริ่มต้น
repo แรกมีแพ็กเกจตัวอย่างที่เราสร้างเอง (`hello`, `l110-extras`) เพื่อทดสอบทั้งสายตั้งแต่ build ถึงติดตั้ง

### 8.1 bash (ติดตั้งผ่าน apt)
- **สถานะในระบบ:** rootfs เริ่มต้นไม่มี bash มีแต่ `/bin/sh` ของ L110 ดังนั้น script ที่ใช้ `#!/bin/bash` จะรันไม่ได้จนกว่าจะติดตั้ง
- **ที่มา:** bash ไม่ได้เขียนเอง เป็นข้อยกเว้นของหลัก "จากศูนย์" โดยใช้ซอร์สของ GNU bash (GPLv3) มา build เป็นแพ็กเกจ `.l110pkg`
- **การ build:** คอมไพล์แบบ static กับ musl (`x86_64-unknown-linux-musl`) เพื่อไม่ต้องพึ่ง glibc ที่ rootfs ไม่มี ทำผ่าน `cargo xtask build-repo` หรือสคริปต์ build แยกใน `packages/bash/`
- **เนื้อหาแพ็กเกจ:** `/bin/bash`, `/etc/bash.bashrc`, `/etc/skel/.bashrc`, `/usr/share/doc/bash/` (ใส่ไฟล์ license และลิงก์ซอร์สตามเงื่อนไข GPL)
- **postinst:** เพิ่ม `/bin/bash` ลง `/etc/shells` (ตัว `/etc/shells` มีอยู่ใน rootfs ตั้งแต่ต้น โดยลิสต์ `/bin/sh` เท่านั้น)
- **เปลี่ยน login shell:** เพิ่มคำสั่ง `chsh` (อยู่ในกลุ่มผู้ใช้ §5) ที่อนุญาตเฉพาะ shell ที่อยู่ใน `/etc/shells`
- **Depends:** ไม่มี (static) จึงทดสอบ resolver แบบง่ายก่อนได้
- **ข้อจำกัดของ shell L110:** ไม่รับประกันฟีเจอร์เฉพาะ bash (array, `[[ ]]`, `${var//a/b}`, `<( )`, `shopt`) ถ้า script ต้องการ ให้ติดตั้ง bash

## 9. Rootfs และการติดตั้งบน WSL

`cargo xtask build-rootfs` สร้าง `l110-rootfs.tar.gz`
- โครง FHS: `/bin /sbin /usr/bin /etc /home /root /tmp /var /proc /sys /dev /run /mnt /opt`
- วาง `l110` (static musl) ที่ `/bin/l110` + symlink คำสั่งทั้งหมด
- ไฟล์ตั้งค่าเริ่มต้น: `/etc/passwd /etc/group /etc/shadow /etc/shells /etc/sudoers /etc/hostname /etc/hosts /etc/os-release /etc/wsl.conf /etc/profile /etc/apt/sources.list`
- `/etc/os-release` ระบุ `NAME="L110"` และ `/etc/wsl.conf` ตั้ง default user และ interop ตามต้องการ
- `/bin/sh` ชี้ไป `l110` (shell) เพราะ WSL เรียกผ่าน `/bin/sh`
- ติดตั้ง: `wsl --import L110 <dir> l110-rootfs.tar.gz --version 2` แล้ว `wsl -d L110` (`scripts/install.ps1` รวมขั้นตอนนี้)
- ข้อควรระวัง: WSL มี init, mount `/proc /sys /dev` ให้เอง จึงไม่ต้องเขียน init เอง แต่ต้องทดสอบการทำงานของ `mount`, `ps`, `/proc` ให้ตรงกับ WSL2

## 10. Milestones

| M | เป้าหมาย | ผลลัพธ์ที่ตรวจสอบได้ |
|---|---|---|
| **M0** | workspace, `l110-core` (registry, parser, help), CI, `xtask` เบื้องต้น | `l110 --help` ใช้ได้ และ test ตรวจ help ผ่าน |
| **M1** | rootfs ขั้นต่ำ + `wsl --import` + shell เริ่มต้น + `echo pwd cd ls cat` | เข้า `wsl -d L110` แล้วเห็น prompt `L110:~/ $` |
| **M2** | shell ครบ (pipe, redirect, expansion, control flow, job control, line editor) | script ทดสอบ shell ผ่านชุด conformance ของเรา |
| **M3** | คำสั่งกลุ่มไฟล์ + ข้อความ (flag ครบตาม manifest) | coverage ของกลุ่มนี้ 100%, golden test ผ่านใน CI |
| **M4** | คำสั่งกลุ่มระบบ + archive/gzip | `ps`, `kill`, `tar`, `gzip` ใช้งานจริงได้ใน WSL |
| **M5** | ระบบผู้ใช้ + `sudo` + `su` + `visudo` | ผ่านชุด test ความปลอดภัยของ sudo และ review |
| **M6** | `apt` + `.l110pkg` + repo + resolver + แพ็กเกจ `bash` + `chsh` | `apt update && apt install hello bash` ทำงานเต็มสาย และ `bash --version` ใช้งานได้ |
| **M7** | ปิด flag ที่เหลือ, ลายเซ็น repo, performance, เอกสาร, release v1.0 | manifest ทั้งหมดเป็น `done`/`n/a` |

## 11. ความเสี่ยงและวิธีรับมือ

| ความเสี่ยง | แนวทาง |
|---|---|
| ปริมาณ flag ครบมาก | ใช้ manifest + coverage ใน CI, ทำเป็นกลุ่มคำสั่ง, ใช้ parser/ spec กลางเพื่อลดโค้ดซ้ำ |
| พฤติกรรมไม่ตรง GNU | golden test เทียบกับของจริงทุก flag |
| ช่องโหว่ใน sudo/su | ทำ threat model, fuzz parser ของ sudoers, review แยก, ไม่ปล่อย release ก่อนผ่าน |
| setuid ไม่ทำงานบน `/mnt/c` | เอกสารระบุชัด และติดตั้ง rootfs ใน ext4 ของ WSL เสมอ |
| ความแตกต่างของ WSL (`/proc`, interop) | ทดสอบบน WSL2 จริงในทุก milestone ไม่พึ่ง CI อย่างเดียว |
| ติดตั้งแพ็กเกจพังกลางทาง | transaction + rollback, lock file |

## 12. ขั้นตอนถัดไป

เริ่ม **M0**: สร้าง workspace, `l110-core` พร้อม parser กับตัวสร้าง help, applet ตัวแรก (`true`, `false`, `echo`) เพื่อพิสูจน์โครง, และ CI
