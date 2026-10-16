# Puri Cleaner

ดูว่าพื้นที่ในเครื่องหายไปไหน แล้วล้างสิ่งที่ลบได้อย่างปลอดภัย — สำหรับ **macOS** และ **Windows**
(See where your disk space went, and reclaim it safely — for macOS and Windows.)

## ดาวน์โหลด (Download)

| ระบบ | ไฟล์ |
|---|---|
| **macOS** 11+ (Apple Silicon และ Intel) | [**PuriCleaner-mac.dmg**](https://github.com/purivaro/PuriCleaner/releases/latest/download/PuriCleaner-mac.dmg) |
| **Windows** 10/11 (64-bit) — ตัวติดตั้ง | [**PuriCleaner-windows-setup.exe**](https://github.com/purivaro/PuriCleaner/releases/latest/download/PuriCleaner-windows-setup.exe) |
| Windows — แบบไม่ต้องติดตั้ง (portable) | [PuriCleaner-windows-portable.zip](https://github.com/purivaro/PuriCleaner/releases/latest/download/PuriCleaner-windows-portable.zip) |
| Windows — เวอร์ชันเปิดในเบราว์เซอร์ | [PuriCleaner-web-windows.exe](https://github.com/purivaro/PuriCleaner/releases/latest/download/PuriCleaner-web-windows.exe) |

ทุกเวอร์ชันและบันทึกการเปลี่ยนแปลงอยู่ที่หน้า [Releases](https://github.com/purivaro/PuriCleaner/releases)

### ติดตั้งบน Mac

1. เปิด `PuriCleaner-mac.dmg` แล้วลาก **Puri Cleaner** ไปใส่โฟลเดอร์ Applications
2. เปิดแอปได้เลย — เซ็นด้วย Developer ID และผ่านการตรวจ (notarize) ของ Apple แล้ว
3. ครั้งแรก macOS จะถามสิทธิ์เข้าถึง Desktop / Documents / Downloads ให้กดอนุญาต
4. ถ้าแอปบอกว่ามีโฟลเดอร์ที่อ่านไม่ได้ ให้กด **เปิดการตั้งค่า** แล้วเปิด Puri Cleaner ใน
   **Full Disk Access** ตัวเลขจะครบขึ้น

### ติดตั้งบน Windows

1. เปิด `PuriCleaner-windows-setup.exe` (ไม่ต้องใช้สิทธิ์ admin เลือกภาษาไทย/อังกฤษได้)
2. ถ้าขึ้นหน้าต่าง **"Windows protected your PC"** ให้กด **More info › Run anyway**
   (ขึ้นเพราะยังไม่ได้เซ็นโค้ด)
3. เปิดจากเมนู Start พิมพ์ "Puri Cleaner" — ถอนการติดตั้งได้ที่ Settings › Apps

แบบ portable: แตกไฟล์ zip แล้วเปิด `Puri Cleaner.exe` (เก็บ `WebView2Loader.dll` ไว้ในโฟลเดอร์เดียวกัน)
แอปใช้ WebView2 ที่มีใน Windows 10/11 อยู่แล้ว ถ้าเครื่องไม่มี ตัวติดตั้งจะดาวน์โหลดให้

## ทำอะไรได้บ้าง

- **แผนที่พื้นที่ (treemap) ทั้งเครื่อง** แยกสีตามประเภท — เอกสารและสื่อ, โปรเจคโค้ด, SDK iOS/Android,
  โมเดล AI, แคชแพ็กเกจ, แอปพลิเคชัน, ระบบ — คลิกเพื่อเจาะเข้าไปดูข้างใน สลับเป็นมุมมอง "ตามการล้าง"
  เพื่อดูว่าแต่ละโฟลเดอร์คืนพื้นที่ได้เท่าไร
  - macOS: โฮมโฟลเดอร์, `/Applications`, `/Library`, Homebrew, ไฟล์ชั่วคราวใน `/private/var/folders`
  - Windows: โฟลเดอร์ผู้ใช้, `Program Files`, `Program Files (x86)`, `ProgramData`, `C:\Windows`
  - ส่วนที่อ่านไม่ได้หรือเป็นไฟล์ระบบ แสดงเป็นกล่องสีเทา
- **คำแนะนำ 2 ระดับ**
  - *ลบได้ปลอดภัย* — แคชและไฟล์ build ที่เครื่องมือสร้าง/โหลดใหม่เองได้ เช่น npm, pip/uv, Gradle, NuGet,
    Xcode DerivedData, `.next`, `target/`, แคชเบราว์เซอร์และแอป Electron, ไฟล์ชั่วคราว, Homebrew เวอร์ชันเก่า,
    ไฟล์อัปเดต Windows ที่ดาวน์โหลดไว้, crash dump
  - *ดูก่อนลบ* — ข้อมูลจริงที่อาจยังต้องใช้ เช่น iOS Simulator และ runtime, Android emulator,
    แอปที่ไม่ได้เปิดเกิน 90 วัน, โมเดล Ollama / Hugging Face, `node_modules` ของโปรเจคที่ไม่ได้แตะเกิน 30 วัน,
    ไฟล์ติดตั้งใน Downloads, ถังขยะ / ถังรีไซเคิล, ไฟล์ไฮเบอร์เนตของ Windows
- **ลบแต่ละอย่างด้วยวิธีที่ถูกต้อง** — ย้ายไปถังขยะ (กู้คืนได้) หรือลบถาวร, ไฟล์ของระบบขอรหัสผ่าน /
  UAC ครั้งเดียว, แอปย้ายลงถังขยะเสมอ, Simulator runtime ลบผ่าน `xcrun simctl`
  หรือกด **สคริปต์** เพื่อคัดลอกคำสั่งไปอ่านและรันเองใน Terminal / PowerShell
- ไทย / English, โหมดมืด/สว่าง

## ปลอดภัยแค่ไหน

- **การสแกนอ่านอย่างเดียว** ไม่ไปตามลิงก์ (symlink / junction) และไม่นับดิสก์อิมเมจที่ mount อยู่ข้างใน
- **ไม่ดาวน์โหลดไฟล์จาก iCloud / OneDrive** — ไฟล์ที่ไม่ได้อยู่ในเครื่องจะถูกข้าม
- ขนาดนับจาก **พื้นที่ที่ใช้บนดิสก์จริง** ไม่ใช่ขนาดไฟล์ และนับ hard link ครั้งเดียว
  (บน Mac ตัวเลขตรงกับ `du` ทุกไบต์ บน Windows ไม่นับ WinSxS ซ้ำ)
- **ลบได้เฉพาะสิ่งที่แอปแนะนำเอง** — ฝั่ง Rust ตรวจซ้ำว่าเป็นรายการในคำแนะนำปัจจุบัน
  และไม่ใช่โฟลเดอร์ต้นทาง (โฮม, /Library, Program Files ฯลฯ) ไม่รับพาธอิสระจากหน้าจอ
- ไม่แนะนำโฟลเดอร์ที่มีตัวโปรแกรมที่กำลังรันอยู่ และข้ามแคชของ Apple (`com.apple.*`)
- เวอร์ชันเบราว์เซอร์ฟังเฉพาะ `127.0.0.1` และทุกคำสั่งต้องมี token สุ่มที่อยู่ในลิงก์

## โครงสร้าง

```
crates/core/      Rust: สแกน, จัดหมวด, กฎคำแนะนำ, การลบ, API แบบ JSON
  src/listing.rs  อ่านทั้งโฟลเดอร์ทีละชุด (macOS: getattrlistbulk, Windows: FileIdBothDirectoryInfo)
  src/scan.rs     เดินโฟลเดอร์แบบขนาน (rayon) → tree.rs
  src/rules.rs    กฎ "ลบได้ปลอดภัย / ดูก่อนลบ" — ตาราง RULES แยก macOS / Windows
  src/sysinfo.rs  ข้อมูลจาก xcrun simctl (runtime) และวันเปิดแอปล่าสุด (macOS)
  src/api.rs      App::call(cmd, args) ใช้ร่วมกันทั้งแอปและเว็บ
crates/web/       เซิร์ฟเวอร์ local สำหรับเปิดในเบราว์เซอร์ (tiny_http)
src-tauri/        แอปเดสก์ท็อป (Tauri 2)
ui/               หน้าจอ HTML/CSS/JS ชุดเดียว ใช้ทั้งแอปและเว็บ (ไม่มี build step)
scripts/          dev-web.sh, build-windows.sh, release.sh, make_icon.py
```

## พัฒนา

ต้องมี Rust (`brew install rustup && rustup default stable`) และ Node สำหรับ Tauri CLI

```bash
./scripts/dev-web.sh                                   # http://127.0.0.1:7171/#t=dev แก้ ui/ แล้วรีโหลดได้เลย
cargo run --release -p puri_core --example scan        # สแกนจาก Terminal ดูผลเร็ว ๆ
cd src-tauri && npx @tauri-apps/cli@2 dev              # แอปโหมดพัฒนา
./scripts/release.sh                                   # สร้างไฟล์ทั้ง Mac และ Windows ลง dist/release/
```

สร้างเวอร์ชัน Windows จาก Mac ได้ (MinGW): `rustup target add x86_64-pc-windows-gnu` และ `brew install mingw-w64 nsis`

แก้ CSS/JS แล้วให้เพิ่มเลข `?v=` ใน `ui/index.html` ด้วย
