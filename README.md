# Puri Cleaner

ดูว่าพื้นที่ในเครื่อง Mac หายไปไหน แล้วล้างสิ่งที่ลบได้อย่างปลอดภัย
(See where your disk space went, and reclaim it safely.)

- **แผนที่พื้นที่ (treemap) ทั้งเครื่อง** — โฮมโฟลเดอร์, `/Applications`, `/Library`, Homebrew และไฟล์ชั่วคราวใน
  `/private/var/folders` แยกสีตามประเภท ส่วนที่เหลือ (ตัว macOS, สแนปช็อต, ที่อ่านไม่ได้) แสดงเป็นกล่องสีเทา
  คลิกเพื่อเจาะเข้าไปดูข้างใน
- **คำแนะนำ 2 ระดับ**
  - *ลบได้ปลอดภัย* — แคชและไฟล์ build ที่เครื่องมือสร้าง/โหลดใหม่เองได้
    (npm, pip/uv, Gradle, Xcode DerivedData, `.next`, `target/`, แคชเบราว์เซอร์ ฯลฯ)
    รวมถึงแคช dyld ของ Simulator, Homebrew เวอร์ชันเก่า, ไฟล์ชั่วคราวของระบบ
  - *ดูก่อนลบ* — ข้อมูลจริงที่อาจยังต้องใช้ (iOS Simulator, Simulator runtime, แอปที่ไม่ได้เปิดเกิน 90 วัน,
    Android emulator, โมเดล Ollama, `node_modules` ของโปรเจคที่ไม่ได้แตะเกิน 30 วัน, ไฟล์ติดตั้งใน Downloads,
    แคช LINE, คลังเสียง GarageBand ฯลฯ)
- ลบแต่ละอย่างด้วยวิธีที่ถูกต้อง: ไฟล์ของระบบ (root) ขอรหัสผ่านเครื่องครั้งเดียว, แอปย้ายลงถังขยะผ่าน Finder เสมอ,
  Simulator runtime ลบผ่าน `xcrun simctl runtime delete`
- **ล้างได้ 2 แบบ**: ย้ายไปถังขยะ (กู้คืนได้) หรือลบถาวร
  หรือกด **สคริปต์** เพื่อคัดลอกคำสั่ง `rm -rf` ไปอ่านและรันเองใน Terminal
- ไทย / English, โหมดมืด/สว่างตามระบบ

## ใช้งาน

### แอป Mac

```bash
open "target/release/bundle/macos/Puri Cleaner.app"
```

ไฟล์ติดตั้งอยู่ที่ `target/release/bundle/dmg/` แอปนี้ไม่ได้เซ็นด้วย Apple Developer ID
เปิดครั้งแรกให้คลิกขวาที่แอป › Open

ถ้าแอปบอกว่ามีโฟลเดอร์ที่อ่านไม่ได้ ให้กด **เปิดการตั้งค่า** แล้วเพิ่ม Puri Cleaner ใน
System Settings › Privacy & Security › Full Disk Access ตัวเลขจะครบขึ้น

### เวอร์ชันเบราว์เซอร์

```bash
cargo run --release -p puri-cleaner-web
```

เปิดเบราว์เซอร์ให้อัตโนมัติ (ฟังเฉพาะ `127.0.0.1` และทุกคำสั่งต้องมี token สุ่มที่อยู่ในลิงก์)
ใส่พาธเพื่อสแกนเฉพาะโฟลเดอร์นั้นแทนทั้งเครื่องได้ เช่น `cargo run --release -p puri-cleaner-web -- ~/Dev`

### บรรทัดคำสั่ง (ดูผลเร็ว ๆ)

```bash
cargo run --release -p puri_core --example scan
```

## ปลอดภัยแค่ไหน

- **การสแกนอ่านอย่างเดียว** และไม่ไปตามลิงก์ (symlink) ไม่ข้ามไปดิสก์อื่น
  และข้ามดิสก์อิมเมจที่ mount อยู่ข้างใน (เช่น Simulator runtime) จึงไม่นับซ้ำ
- **ไม่ดาวน์โหลดไฟล์จาก iCloud** — โฟลเดอร์/ไฟล์ที่ไม่ได้อยู่ในเครื่องจะถูกข้าม
  (ตั้ง `IOPOL_MATERIALIZE_DATALESS_FILES_OFF` ทั้งโปรเซส)
- ขนาดนับจาก **พื้นที่ที่ใช้บนดิสก์จริง** (`st_blocks`) ไม่ใช่ขนาดไฟล์ จึงไม่หลอกตาเรื่อง sparse file
  และนับ hard link ครั้งเดียว ตัวเลขตรงกับ `du` ทุกไบต์
- **ลบได้เฉพาะสิ่งที่แอปแนะนำเอง** — ฝั่ง Rust ตรวจซ้ำว่าเป็นรายการในคำแนะนำปัจจุบัน
  และไม่ใช่โฟลเดอร์ต้นทาง (โฮม, /Library ฯลฯ) ไม่รับพาธอิสระจากหน้าจอ
- ไม่แนะนำโฟลเดอร์ที่มีตัวโปรแกรมที่กำลังรันอยู่ (เช่น `target/` ของโปรเจคนี้เอง)
- การลบไม่ไปตาม symlink ข้างใน (ลบแค่ตัวลิงก์ ไม่แตะปลายทาง)
- ข้ามแคชของ Apple (`com.apple.*`) และ node_modules ของโปรเจคที่ยังทำงานอยู่

## โครงสร้าง

```
crates/core/      Rust: สแกน (getattrlistbulk), จัดหมวด, กฎคำแนะนำ, การลบ, API แบบ JSON
  src/listing.rs  อ่านทั้งโฟลเดอร์ใน syscall เดียว (เร็วกว่า lstat ทีละไฟล์ ~13 เท่า)
  src/scan.rs     เดินโฟลเดอร์แบบขนาน (rayon) → tree.rs
  src/rules.rs    กฎ "ลบได้ปลอดภัย / ดูก่อนลบ" — เพิ่มกฎใหม่ที่ตาราง RULES
  src/sysinfo.rs  ข้อมูลจาก xcrun simctl (runtime) และวันเปิดแอปล่าสุด (Spotlight + atime)
  src/api.rs      App::call(cmd, args) ใช้ร่วมกันทั้งแอปและเว็บ
crates/web/       เซิร์ฟเวอร์ local สำหรับเปิดในเบราว์เซอร์ (tiny_http)
src-tauri/        แอป Mac (Tauri 2)
ui/               หน้าจอ HTML/CSS/JS ชุดเดียว ใช้ทั้งสองแบบ (ไม่มี build step)
scripts/          dev-web.sh (รันเว็บจาก ui/ บนดิสก์), make_icon.py
```

## พัฒนา

ต้องมี Rust (`brew install rustup && rustup default stable`) และ Node สำหรับ Tauri CLI

```bash
./scripts/dev-web.sh                    # http://127.0.0.1:7171/#t=dev แก้ ui/ แล้วรีโหลดได้เลย
npx @tauri-apps/cli@2 dev               # แอป Mac โหมดพัฒนา (รันใน src-tauri/)
npx @tauri-apps/cli@2 build             # สร้าง .app และ .dmg (รันใน src-tauri/)
```

แก้ CSS/JS แล้วให้เพิ่มเลข `?v=` ใน `ui/index.html` ด้วย
