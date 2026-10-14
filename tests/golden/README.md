# Golden tests

เทียบ output ของ L110 กับ GNU coreutils จริง (`/usr/bin/<cmd>`) ทีละ case

- ไฟล์ `<cmd>.cases` หนึ่งบรรทัดต่อหนึ่ง case: `[ENV=VAL]... ARGUMENT...` (quoting แบบ shell อย่างง่าย
  ได้แก่ `'...'`, `"..."`, `\x`) ตัวแรกที่ไม่ใช่ `ENV=VAL` คือชื่อคำสั่ง
- เปรียบเทียบ stdout, stderr (ตามไบต์) และ exit status
- บรรทัดที่ขึ้นต้นด้วย `~ ` เทียบเฉพาะ exit status (ใช้กับ `--help`/`--version`
  ที่ข้อความของ L110 ตั้งใจให้ต่างจาก GNU)
- `#` คือ comment

รัน: `cargo test -p l110 --test golden`
ถ้าไม่มี GNU coreutils ในเครื่อง test จะข้าม (พิมพ์เหตุผล) ยกเว้นตั้ง `L110_REQUIRE_GNU=1`
ซึ่งทำให้ล้มแทน (CI ตั้งค่านี้)
