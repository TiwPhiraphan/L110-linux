# L110-linux

userland ของ Linux ที่เขียนเองด้วย Rust ทั้งหมด แพ็กเป็น rootfs สำหรับ `wsl --import`
(ใช้ kernel ของ WSL) รายละเอียดและ milestone ดู [PLAN.md](PLAN.md)

## สถานะ

M0 (workspace, `l110-core`, `echo`/`true`/`false`, CI, `xtask`) ดู "Milestones" ใน PLAN.md

## พัฒนา

```sh
cargo test --workspace            # unit + integration + golden (เทียบ GNU coreutils)
cargo xtask coverage              # สรุปความครอบคลุมของ flag (flags/*.toml)
cargo xtask coverage --check      # ล้มถ้าความครอบคลุมลดลงหรือ manifest ไม่ตรง spec
cargo run -p l110 -- --list       # รายการคำสั่งทั้งหมด
cargo run -p l110 -- echo hello   # รันคำสั่งผ่าน binary เดียว
```

binary เดียว `l110` เป็น multi-call: เลือกคำสั่งจากชื่อที่ถูกเรียก (symlink) หรือ argument แรก
เพิ่มคำสั่งใหม่ = เขียน `CommandSpec` + `main` แล้วลงทะเบียนใน `APPLETS` ของ crate กลุ่ม,
เพิ่ม `flags/<cmd>.toml`, `tests/golden/<cmd>.cases` แล้วรัน `cargo xtask coverage --update`
