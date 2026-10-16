//! Cleanup recommendations: what is safe to delete, and what to look at first.
//!
//! "Safe" means the owning tool rebuilds or re-downloads it on demand (caches,
//! build output). "Review" means it is real data the user may still want
//! (models, emulators, old projects' dependencies, installers).
//!
//! Rules run in table order and each folder is claimed by at most one rule,
//! so specific rules must come before broad ones (e.g. Homebrew before the
//! catch-all "app caches").

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::sysinfo::SysInfo;
use crate::tree::Tree;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Safe,
    Review,
}

// Each platform's table uses only some finders.
#[allow(dead_code)]
enum Find {
    /// Home-relative paths; each existing one is an item. A `*` segment
    /// matches any folder name.
    Paths(&'static [&'static str]),
    /// Every child of a folder, except names starting with these prefixes.
    ChildrenOf(&'static str, &'static [&'static str]),
    /// Matched while walking project folders (see `pattern_scan`).
    Pattern,
    /// Installer files sitting in ~/Downloads.
    Installers,
    /// Big files in ~/Downloads untouched for a long time.
    OldDownloads,
    /// One item per iOS simulator, labelled with its device name.
    Simulators,
    /// Installed simulator runtimes (disk images managed by `simctl`).
    Runtimes,
    /// Homebrew formula versions that are no longer linked.
    BrewOld,
    /// Apps in /Applications not opened for a long time.
    UnusedApps,
    /// Old files in the per-user temp folder and app caches in /var/folders.
    UserTemp,
    /// The Windows Recycle Bin (measured and emptied through the shell).
    RecycleBin,
}

struct RuleDef {
    id: &'static str,
    level: Level,
    title: [&'static str; 2],
    desc: [&'static str; 2],
    hint: &'static str,
    command: Option<&'static str>,
    find: Find,
}

const IDLE_DAYS: i64 = 30;
const OLD_DOWNLOAD_DAYS: i64 = 90;
const OLD_DOWNLOAD_MIN: u64 = 100 * 1000 * 1000;
const MIN_RULE_SIZE: u64 = 1000 * 1000;

use Find::*;
use Level::*;

#[cfg(not(windows))]
const RULES: &[RuleDef] = &[
    // ─── Safe: tools rebuild or re-download these on demand ───
    RuleDef {
        id: "npm",
        level: Safe,
        title: ["แคช npm", "npm cache"],
        desc: [
            "แพ็กเกจที่ npm ดาวน์โหลดเก็บไว้ ลบได้ npm จะโหลดใหม่เมื่อจำเป็น",
            "Packages npm downloaded before. npm fetches them again when needed.",
        ],
        hint: "~/.npm",
        command: Some("npm cache clean --force"),
        find: Paths(&[".npm/_cacache", ".npm/_npx", ".npm/_logs", ".npm/_prebuilds"]),
    },
    RuleDef {
        id: "js-build",
        level: Safe,
        title: ["ไฟล์ build ของ Next.js / Turbo", "Next.js & Turbo build output"],
        desc: [
            "โฟลเดอร์ .next .turbo .nuxt .svelte-kit .parcel-cache ในโปรเจค สร้างใหม่ทุกครั้งที่ build/dev",
            ".next, .turbo, .nuxt, .svelte-kit and .parcel-cache folders. Rebuilt on the next build or dev run.",
        ],
        hint: "*/.next, .turbo",
        command: None,
        find: Pattern,
    },
    RuleDef {
        id: "yarn-pnpm-bun",
        level: Safe,
        title: ["แคช Yarn / pnpm / Bun", "Yarn, pnpm & Bun caches"],
        desc: [
            "ที่เก็บแพ็กเกจกลางของ Yarn, pnpm และ Bun ลบแล้วจะดาวน์โหลดใหม่ตอนติดตั้ง",
            "Shared package stores. Re-downloaded on the next install.",
        ],
        hint: "~/Library/Caches/Yarn, ~/Library/pnpm/store",
        command: Some("yarn cache clean; pnpm store prune"),
        find: Paths(&[
            "Library/Caches/Yarn",
            ".yarn/berry/cache",
            ".cache/yarn",
            "Library/pnpm/store",
            ".pnpm-store",
            ".local/share/pnpm/store",
            ".bun/install/cache",
        ]),
    },
    RuleDef {
        id: "rust-target",
        level: Safe,
        title: ["ไฟล์ build ของ Rust (target)", "Rust build output (target/)"],
        desc: [
            "โฟลเดอร์ target ข้างไฟล์ Cargo.toml สร้างใหม่ได้ด้วย cargo build (ครั้งแรกจะช้าหน่อย)",
            "target/ folders next to Cargo.toml. cargo build recreates them (the first build is slower).",
        ],
        hint: "*/target",
        command: Some("cargo clean"),
        find: Pattern,
    },
    RuleDef {
        id: "cargo",
        level: Safe,
        title: ["แคช Cargo", "Cargo registry cache"],
        desc: [
            "crate ที่ Cargo ดาวน์โหลดและแตกไฟล์ไว้ โหลดใหม่อัตโนมัติตอน build",
            "Downloaded and unpacked crates. Fetched again on the next build.",
        ],
        hint: "~/.cargo/registry",
        command: None,
        find: Paths(&[".cargo/registry/cache", ".cargo/registry/src", ".cargo/git/checkouts"]),
    },
    RuleDef {
        id: "gradle",
        level: Safe,
        title: ["แคช Gradle", "Gradle caches"],
        desc: [
            "ไลบรารีและ Gradle wrapper ที่ดาวน์โหลดไว้ build ครั้งถัดไปจะโหลดใหม่",
            "Downloaded libraries and Gradle wrappers. The next build downloads them again.",
        ],
        hint: "~/.gradle",
        command: None,
        find: Paths(&[".gradle/caches", ".gradle/wrapper/dists", ".gradle/daemon", ".gradle/native", ".gradle/.tmp"]),
    },
    RuleDef {
        id: "maven",
        level: Safe,
        title: ["แคช Maven", "Maven repository"],
        desc: ["ไลบรารี Java ที่ Maven ดาวน์โหลดไว้", "Java libraries downloaded by Maven."],
        hint: "~/.m2/repository",
        command: None,
        find: Paths(&[".m2/repository"]),
    },
    RuleDef {
        id: "xcode-derived",
        level: Safe,
        title: ["Xcode DerivedData", "Xcode DerivedData"],
        desc: [
            "ไฟล์ build ชั่วคราวและ index ของ Xcode สร้างใหม่ตอนเปิดโปรเจค",
            "Xcode's intermediate build files and indexes. Recreated when you open a project.",
        ],
        hint: "~/Library/Developer/Xcode/DerivedData",
        command: None,
        find: Paths(&["Library/Developer/Xcode/DerivedData"]),
    },
    RuleDef {
        id: "xcode-support",
        level: Safe,
        title: ["ข้อมูลอุปกรณ์ของ Xcode และแคช Simulator", "Xcode device support & simulator caches"],
        desc: [
            "สัญลักษณ์ดีบักของ iOS แต่ละเวอร์ชัน และแคชของ Simulator Xcode คัดลอกใหม่เมื่อเสียบเครื่อง",
            "Debug symbols per iOS version and simulator caches. Xcode copies them again when a device connects.",
        ],
        hint: "~/Library/Developer/Xcode/iOS DeviceSupport",
        command: None,
        find: Paths(&[
            "Library/Developer/Xcode/iOS DeviceSupport",
            "Library/Developer/Xcode/watchOS DeviceSupport",
            "Library/Developer/Xcode/tvOS DeviceSupport",
            "Library/Developer/Xcode/visionOS DeviceSupport",
            "Library/Developer/CoreSimulator/Caches",
        ]),
    },
    RuleDef {
        id: "sim-dyld",
        level: Safe,
        title: ["แคช dyld ของ Simulator", "Simulator dyld caches"],
        desc: [
            "ไลบรารีที่ Simulator เตรียมไว้ให้เปิดเร็ว สร้างใหม่เองตอนเปิด Simulator ครั้งถัดไป เป็นไฟล์ของระบบ จึงต้องใส่รหัสผ่านเครื่อง",
            "Libraries prepared so simulators start fast. Rebuilt the next time a simulator boots. System-owned, so your Mac password is needed.",
        ],
        hint: "/Library/Developer/CoreSimulator/Caches",
        command: None,
        find: Paths(&["/Library/Developer/CoreSimulator/Caches"]),
    },
    RuleDef {
        id: "brew-old",
        level: Safe,
        title: ["Homebrew เวอร์ชันเก่า", "Old Homebrew versions"],
        desc: [
            "เวอร์ชันเก่าของโปรแกรมที่อัปเดตแล้วแต่ยังค้างอยู่ ตัวที่ใช้งานจริงไม่ถูกแตะ",
            "Older versions left behind after upgrades. The versions in use are not touched.",
        ],
        hint: "/opt/homebrew/Cellar/*/<old>",
        command: Some("brew cleanup --prune=all"),
        find: BrewOld,
    },
    RuleDef {
        id: "user-temp",
        level: Safe,
        title: ["ไฟล์ชั่วคราวและแคชของระบบ", "Temporary files & system caches"],
        desc: [
            "ไฟล์ชั่วคราวเก่ากว่า 3 วัน และแคชของแอปใน /private/var/folders (ข้ามของ Apple)",
            "Temporary files older than 3 days and app caches in /private/var/folders (Apple's are skipped).",
        ],
        hint: "/private/var/folders/…/T, …/C",
        command: None,
        find: UserTemp,
    },
    RuleDef {
        id: "homebrew",
        level: Safe,
        title: ["แคช Homebrew", "Homebrew cache"],
        desc: [
            "ไฟล์ติดตั้งที่ Homebrew ดาวน์โหลดไว้ โปรแกรมที่ติดตั้งแล้วไม่ได้รับผลกระทบ",
            "Downloaded bottles and source archives. Installed formulae are not affected.",
        ],
        hint: "~/Library/Caches/Homebrew",
        command: Some("brew cleanup --prune=all"),
        find: Paths(&["Library/Caches/Homebrew"]),
    },
    RuleDef {
        id: "python",
        level: Safe,
        title: ["แคช pip / uv / Poetry", "pip, uv & Poetry caches"],
        desc: [
            "แพ็กเกจ Python ที่ดาวน์โหลดไว้ ไม่กระทบ virtualenv ที่ติดตั้งแล้ว",
            "Downloaded Python packages. Existing virtualenvs keep working.",
        ],
        hint: "~/.cache/uv, ~/Library/Caches/pip",
        command: Some("uv cache clean; pip cache purge"),
        find: Paths(&[
            "Library/Caches/pip",
            ".cache/pip",
            ".cache/uv",
            "Library/Caches/pypoetry/cache",
            "Library/Caches/pypoetry/artifacts",
        ]),
    },
    RuleDef {
        id: "browsers",
        level: Safe,
        title: ["เบราว์เซอร์ทดสอบ (Puppeteer / Playwright)", "Puppeteer & Playwright browsers"],
        desc: [
            "Chrome/Firefox ที่เครื่องมือทดสอบดาวน์โหลดมา ติดตั้งใหม่ได้ด้วยคำสั่ง install",
            "Browsers downloaded by test tools. Reinstall with their install command.",
        ],
        hint: "~/.cache/puppeteer, ~/Library/Caches/ms-playwright",
        command: Some("npx playwright install"),
        find: Paths(&[".cache/puppeteer", "Library/Caches/ms-playwright", ".cache/ms-playwright", "Library/Caches/Cypress"]),
    },
    RuleDef {
        id: "lang-caches",
        level: Safe,
        title: ["แคช Go / Composer / CocoaPods", "Go, Composer & CocoaPods caches"],
        desc: ["แคชคอมไพล์และแพ็กเกจที่โหลดใหม่ได้", "Build and package caches that are fetched again."],
        hint: "~/Library/Caches/go-build, composer, CocoaPods",
        command: Some("go clean -cache"),
        find: Paths(&[
            "Library/Caches/go-build",
            "Library/Caches/composer",
            ".composer/cache",
            ".cache/composer",
            "Library/Caches/CocoaPods",
        ]),
    },
    RuleDef {
        id: "xctest",
        level: Safe,
        title: ["สำเนา Simulator ที่ Xcode ใช้เทสต์", "Xcode test device clones"],
        desc: [
            "Simulator ที่ Xcode โคลนไว้รันเทสต์แบบขนาน สร้างใหม่ทุกครั้งที่รันเทสต์",
            "Simulator clones Xcode makes for parallel testing. Recreated on the next test run.",
        ],
        hint: "~/Library/Developer/XCTestDevices",
        command: None,
        find: Paths(&["Library/Developer/XCTestDevices"]),
    },
    RuleDef {
        id: "mobile-build",
        level: Safe,
        title: ["ไฟล์ build ของ Flutter / Android / Swift", "Flutter, Android & Swift build output"],
        desc: [
            "โฟลเดอร์ build, .dart_tool, .gradle, .cxx และ .build ในโปรเจค สร้างใหม่ตอน build",
            "build, .dart_tool, .gradle, .cxx and .build folders in projects. Rebuilt on the next build.",
        ],
        hint: "*/build, .dart_tool, .gradle",
        command: Some("flutter clean"),
        find: Pattern,
    },
    RuleDef {
        id: "app-web-caches",
        level: Safe,
        title: ["แคชของเบราว์เซอร์และแอป Electron", "Browser & Electron app caches"],
        desc: [
            "แคชหน้าเว็บ โค้ด และกราฟิกของ Chrome, Edge, VS Code, Slack, Claude ฯลฯ ไม่กระทบรหัสผ่าน ประวัติ หรือการตั้งค่า",
            "Web, code and GPU caches of Chrome, Edge, VS Code, Slack, Claude and others. Passwords, history and settings are untouched.",
        ],
        hint: "~/Library/Application Support/*/Cache",
        command: None,
        find: Paths(&[
            "Library/Application Support/*/Cache",
            "Library/Application Support/*/Code Cache",
            "Library/Application Support/*/GPUCache",
            "Library/Application Support/*/DawnCache",
            "Library/Application Support/*/DawnGraphiteCache",
            "Library/Application Support/*/DawnWebGPUCache",
            "Library/Application Support/*/GraphiteDawnCache",
            "Library/Application Support/*/CachedData",
            "Library/Application Support/*/CachedExtensionVSIXs",
            "Library/Application Support/*/Service Worker/CacheStorage",
            "Library/Application Support/Google/Chrome/*/Service Worker/CacheStorage",
            "Library/Application Support/Google/Chrome/*/Code Cache",
            "Library/Application Support/Google/Chrome/*/GPUCache",
            "Library/Application Support/Google/Chrome/component_crx_cache",
            "Library/Application Support/Google/Chrome/extensions_crx_cache",
            "Library/Application Support/Microsoft Edge/*/Service Worker/CacheStorage",
            "Library/Application Support/Microsoft Edge/*/Code Cache",
            "Library/Application Support/Microsoft Edge/*/GPUCache",
            "Library/Application Support/BraveSoftware/Brave-Browser/*/Service Worker/CacheStorage",
            "Library/Application Support/BraveSoftware/Brave-Browser/*/Code Cache",
            "Library/Application Support/BraveSoftware/Brave-Browser/*/GPUCache",
        ]),
    },
    RuleDef {
        id: "logs",
        level: Safe,
        title: ["ไฟล์ล็อกของแอป", "App logs"],
        desc: ["ล็อกที่แอปต่าง ๆ เขียนทิ้งไว้", "Log files written by apps."],
        hint: "~/Library/Logs",
        command: None,
        find: Paths(&["Library/Logs", "Library/Application Support/Code/logs", "Library/Application Support/Cursor/logs"]),
    },
    RuleDef {
        id: "app-caches",
        level: Safe,
        title: ["แคชของแอปอื่น ๆ", "Other app caches"],
        desc: [
            "แคชของแอปทั่วไปใน ~/Library/Caches แอปจะสร้างใหม่เอง (ข้ามของ Apple)",
            "General app caches. Apps rebuild them (Apple's own caches are skipped).",
        ],
        hint: "~/Library/Caches",
        command: None,
        find: ChildrenOf("Library/Caches", &["com.apple.", "CloudKit", "FamilyCircle", "GeoServices"]),
    },
    // ─── Review: real data the user might still want ───
    RuleDef {
        id: "simulators",
        level: Review,
        title: ["iOS Simulator", "iOS simulators"],
        desc: [
            "เครื่องจำลอง iPhone/iPad แต่ละเครื่อง พร้อมแอปและข้อมูลข้างใน ลบเครื่องที่ไม่ใช้ได้ Xcode สร้างใหม่ได้",
            "Each simulated iPhone/iPad with its apps and data. Remove the ones you do not use; Xcode can create them again.",
        ],
        hint: "~/Library/Developer/CoreSimulator/Devices",
        command: Some("xcrun simctl delete unavailable"),
        find: Simulators,
    },
    RuleDef {
        id: "sim-runtimes",
        level: Review,
        title: ["ระบบปฏิบัติการของ Simulator", "Simulator runtimes"],
        desc: [
            "iOS / tvOS / watchOS ที่ใช้รันเครื่องจำลอง ลบเวอร์ชันที่ไม่ใช้แล้ว ถ้าต้องการอีก Xcode ดาวน์โหลดใหม่ได้ (ไฟล์ใหญ่) Simulator ที่ใช้เวอร์ชันนั้นจะใช้ไม่ได้",
            "The iOS / tvOS / watchOS versions simulators run on. Remove ones you no longer need; Xcode can download them again (large). Simulators using them stop working.",
        ],
        hint: "xcrun simctl runtime list",
        command: Some("xcrun simctl runtime delete <id>"),
        find: Runtimes,
    },
    RuleDef {
        id: "unused-apps",
        level: Review,
        title: ["แอปที่ไม่ได้เปิดเกิน 90 วัน", "Apps not opened in 90 days"],
        desc: [
            "แอปใน /Applications ที่ไม่ได้เปิดมานาน จะถูกย้ายไปถังขยะเสมอ กู้คืนได้ ตรวจว่าไม่ต้องใช้แล้วก่อน",
            "Apps in /Applications you have not opened for a long time. Always moved to the Trash, so you can restore them.",
        ],
        hint: "/Applications",
        command: None,
        find: UnusedApps,
    },
    RuleDef {
        id: "audio-library",
        level: Review,
        title: ["คลังเสียงของ GarageBand / Logic", "GarageBand & Logic sound library"],
        desc: [
            "เสียงเครื่องดนตรีและ loop ถ้าไม่ได้ทำเพลงลบได้ แอปจะดาวน์โหลดใหม่เมื่อต้องใช้ ต้องใส่รหัสผ่านเครื่อง",
            "Instrument sounds and loops. Safe to remove if you do not make music; the apps download them again. Needs your Mac password.",
        ],
        hint: "/Library/Application Support/GarageBand",
        command: None,
        find: Paths(&[
            "/Library/Application Support/GarageBand",
            "/Library/Application Support/Logic",
            "/Library/Audio/Apple Loops",
        ]),
    },
    RuleDef {
        id: "android",
        level: Review,
        title: ["Android emulator และ system image", "Android emulators & system images"],
        desc: [
            "เครื่องจำลองและอิมเมจระบบ Android ลบแล้วต้องดาวน์โหลดใหม่ผ่าน Android Studio ถ้าจะใช้อีก",
            "Emulator disks and system images. Download again from Android Studio if you need them.",
        ],
        hint: "~/.android/avd, ~/Library/Android/sdk/system-images",
        command: None,
        find: Paths(&["Library/Android/sdk/system-images", ".android/avd", ".android/cache"]),
    },
    RuleDef {
        id: "ollama",
        level: Review,
        title: ["โมเดล AI ของ Ollama", "Ollama models"],
        desc: [
            "โมเดลภาษาที่ดาวน์โหลดไว้ ถ้าจะลบบางตัวให้ใช้ ollama rm <ชื่อโมเดล>",
            "Downloaded language models. To remove just one, use ollama rm <model>.",
        ],
        hint: "~/.ollama/models",
        command: Some("ollama list   # then: ollama rm <model>"),
        find: Paths(&[".ollama/models"]),
    },
    RuleDef {
        id: "ai-models",
        level: Review,
        title: ["โมเดล AI อื่น ๆ (Hugging Face / LM Studio / Chrome)", "Other AI models (Hugging Face, LM Studio, Chrome)"],
        desc: [
            "โมเดลและ dataset ที่ดาวน์โหลดไว้ รวมถึงโมเดล AI ในเครื่องของ Chrome โหลดใหม่ได้แต่ไฟล์ใหญ่",
            "Downloaded models and datasets, including Chrome's on-device AI model. Re-downloadable, but large.",
        ],
        hint: "~/.cache/huggingface, ~/.lmstudio/models",
        command: None,
        find: Paths(&[
            ".cache/huggingface",
            ".lmstudio/models",
            ".cache/lm-studio/models",
            ".cache/torch",
            ".cache/whisper",
            "Library/Application Support/Google/Chrome/OptGuideOnDeviceModel",
        ]),
    },
    RuleDef {
        id: "claude-vm",
        level: Review,
        title: ["อิมเมจเครื่องเสมือนของ Claude", "Claude desktop VM image"],
        desc: [
            "ระบบที่แอป Claude ใช้รันงานแบบ agent ลบได้ แอปจะดาวน์โหลดใหม่ตอนใช้ครั้งถัดไป",
            "The VM the Claude app uses for agent tasks. The app downloads it again the next time it is needed.",
        ],
        hint: "~/Library/Application Support/Claude/vm_bundles",
        command: None,
        find: Paths(&["Library/Application Support/Claude/vm_bundles"]),
    },
    RuleDef {
        id: "line",
        level: Review,
        title: ["แคชรูปและไฟล์ของ LINE", "LINE media cache"],
        desc: [
            "รูป วิดีโอ และไฟล์ในแชตที่เคยเปิด ลบได้ แต่ไฟล์เก่าที่หมดอายุบนเซิร์ฟเวอร์แล้วจะเปิดไม่ได้อีก",
            "Photos, videos and files from chats you opened. Old ones that expired on LINE's servers cannot be opened again.",
        ],
        hint: "~/Library/Group Containers/…line.mac/…/Caches",
        command: None,
        find: Paths(&["Library/Group Containers/*.jp.naver.line.mac/Real/Library/Data/Caches"]),
    },
    RuleDef {
        id: "docker",
        level: Review,
        title: ["ดิสก์ของ Docker", "Docker disk image"],
        desc: [
            "เก็บ image, container และ volume ทั้งหมดของ Docker แนะนำใช้ docker system prune แทนการลบไฟล์",
            "Holds all Docker images, containers and volumes. Prefer docker system prune over deleting it.",
        ],
        hint: "~/Library/Containers/com.docker.docker",
        command: Some("docker system prune -a"),
        find: Paths(&["Library/Containers/com.docker.docker/Data/vms"]),
    },
    RuleDef {
        id: "idle-deps",
        level: Review,
        title: ["node_modules / Pods ในโปรเจคที่ไม่ได้แตะ", "node_modules & Pods in idle projects"],
        desc: [
            "โปรเจคที่ไม่มีไฟล์ถูกแก้เกิน 30 วัน ลบได้แล้วสั่ง npm install / pod install ใหม่เมื่อกลับมาทำ",
            "Projects with no edits for 30+ days. Run npm install / pod install again when you come back.",
        ],
        hint: "*/node_modules, */Pods",
        command: None,
        find: Pattern,
    },
    RuleDef {
        id: "idle-venv",
        level: Review,
        title: ["Python virtualenv ในโปรเจคที่ไม่ได้แตะ", "Python virtualenvs in idle projects"],
        desc: [
            "โฟลเดอร์ .venv / venv ของโปรเจคที่ไม่ได้แก้เกิน 30 วัน สร้างใหม่ได้จาก requirements",
            ".venv / venv folders in projects idle for 30+ days. Recreate from requirements.",
        ],
        hint: "*/.venv",
        command: None,
        find: Pattern,
    },
    RuleDef {
        id: "worktrees",
        level: Review,
        title: ["git worktree ที่ Claude Code สร้างไว้", "Claude Code worktrees"],
        desc: [
            "สำเนาโปรเจคที่ใช้ทำงานคู่ขนาน ตรวจว่า merge/push งานแล้วก่อนลบ",
            "Parallel working copies. Make sure their work is merged or pushed first.",
        ],
        hint: "*/.claude/worktrees",
        command: Some("git worktree prune"),
        find: Pattern,
    },
    RuleDef {
        id: "xcode-archives",
        level: Review,
        title: ["Xcode Archives", "Xcode Archives"],
        desc: [
            "แอปที่ archive ไว้ตอนส่ง store เก็บไว้ถ้ายังต้องใช้ dSYM ดู crash",
            "Archived app builds. Keep the ones whose dSYMs you need for crash reports.",
        ],
        hint: "~/Library/Developer/Xcode/Archives",
        command: None,
        find: Paths(&["Library/Developer/Xcode/Archives"]),
    },
    RuleDef {
        id: "ios-backups",
        level: Review,
        title: ["แบ็กอัป iPhone / iPad", "iPhone & iPad backups"],
        desc: [
            "แบ็กอัปเครื่องที่ทำผ่าน Finder ลบเฉพาะเครื่องที่ไม่ใช้แล้ว",
            "Device backups made with Finder. Remove only devices you no longer use.",
        ],
        hint: "~/Library/Application Support/MobileSync/Backup",
        command: None,
        find: ChildrenOf("Library/Application Support/MobileSync/Backup", &[]),
    },
    RuleDef {
        id: "installers",
        level: Review,
        title: ["ไฟล์ติดตั้งใน Downloads", "Installers in Downloads"],
        desc: [
            "ไฟล์ .dmg .pkg .iso .xip ที่ติดตั้งเสร็จแล้วส่วนใหญ่ไม่ต้องเก็บ",
            ".dmg, .pkg, .iso and .xip files. Usually not needed once installed.",
        ],
        hint: "~/Downloads/*.dmg, *.pkg",
        command: None,
        find: Installers,
    },
    RuleDef {
        id: "old-downloads",
        level: Review,
        title: ["ไฟล์ใหญ่เก่า ๆ ใน Downloads", "Large old downloads"],
        desc: [
            "ไฟล์เกิน 100 MB ที่ไม่ได้แตะเกิน 90 วัน",
            "Files over 100 MB untouched for 90+ days.",
        ],
        hint: "~/Downloads",
        command: None,
        find: OldDownloads,
    },
    RuleDef {
        id: "dot-cache",
        level: Review,
        title: ["แคชอื่น ๆ ใน ~/.cache", "Other caches in ~/.cache"],
        desc: [
            "แคชของเครื่องมือ command line ส่วนใหญ่ลบได้ แต่บางตัวเก็บข้อมูลที่โหลดนาน",
            "Command-line tool caches. Mostly disposable, but some take long to rebuild.",
        ],
        hint: "~/.cache",
        command: None,
        find: ChildrenOf(".cache", &[]),
    },
    RuleDef {
        id: "trash",
        level: Review,
        title: ["ถังขยะ", "Trash"],
        desc: [
            "ไฟล์ที่ย้ายลงถังขยะยังกินพื้นที่อยู่จนกว่าจะล้างถัง",
            "Trashed files still use space until the Trash is emptied.",
        ],
        hint: "~/.Trash",
        command: None,
        find: ChildrenOf(".Trash", &[".DS_Store"]),
    },
];

/// Windows: same ideas, Windows locations. Paths with a leading `/` are
/// relative to the system drive.
#[cfg(windows)]
const RULES: &[RuleDef] = &[
    // ─── Safe ───
    RuleDef {
        id: "npm",
        level: Safe,
        title: ["แคช npm", "npm cache"],
        desc: [
            "แพ็กเกจที่ npm ดาวน์โหลดเก็บไว้ ลบได้ npm จะโหลดใหม่เมื่อจำเป็น",
            "Packages npm downloaded before. npm fetches them again when needed.",
        ],
        hint: "%LOCALAPPDATA%\\npm-cache",
        command: Some("npm cache clean --force"),
        find: Paths(&["AppData/Local/npm-cache/_cacache", "AppData/Local/npm-cache/_npx", "AppData/Local/npm-cache/_logs"]),
    },
    RuleDef {
        id: "js-build",
        level: Safe,
        title: ["ไฟล์ build ของ Next.js / Turbo", "Next.js & Turbo build output"],
        desc: [
            "โฟลเดอร์ .next .turbo .nuxt .svelte-kit .parcel-cache ในโปรเจค สร้างใหม่ทุกครั้งที่ build/dev",
            ".next, .turbo, .nuxt, .svelte-kit and .parcel-cache folders. Rebuilt on the next build or dev run.",
        ],
        hint: "*\\.next, .turbo",
        command: None,
        find: Pattern,
    },
    RuleDef {
        id: "yarn-pnpm-bun",
        level: Safe,
        title: ["แคช Yarn / pnpm / Bun", "Yarn, pnpm & Bun caches"],
        desc: [
            "ที่เก็บแพ็กเกจกลางของ Yarn, pnpm และ Bun ลบแล้วจะดาวน์โหลดใหม่ตอนติดตั้ง",
            "Shared package stores. Re-downloaded on the next install.",
        ],
        hint: "%LOCALAPPDATA%\\Yarn, pnpm",
        command: Some("yarn cache clean; pnpm store prune"),
        find: Paths(&["AppData/Local/Yarn/Cache", "AppData/Local/pnpm/store", "AppData/Local/pnpm-cache", ".bun/install/cache"]),
    },
    RuleDef {
        id: "rust-target",
        level: Safe,
        title: ["ไฟล์ build ของ Rust (target)", "Rust build output (target\\)"],
        desc: [
            "โฟลเดอร์ target ข้างไฟล์ Cargo.toml สร้างใหม่ได้ด้วย cargo build",
            "target folders next to Cargo.toml. cargo build recreates them.",
        ],
        hint: "*\\target",
        command: Some("cargo clean"),
        find: Pattern,
    },
    RuleDef {
        id: "cargo",
        level: Safe,
        title: ["แคช Cargo", "Cargo registry cache"],
        desc: ["crate ที่ Cargo ดาวน์โหลดไว้ โหลดใหม่อัตโนมัติ", "Downloaded crates. Fetched again on the next build."],
        hint: "%USERPROFILE%\\.cargo\\registry",
        command: None,
        find: Paths(&[".cargo/registry/cache", ".cargo/registry/src", ".cargo/git/checkouts"]),
    },
    RuleDef {
        id: "gradle",
        level: Safe,
        title: ["แคช Gradle", "Gradle caches"],
        desc: ["ไลบรารีและ Gradle wrapper ที่ดาวน์โหลดไว้", "Downloaded libraries and Gradle wrappers."],
        hint: "%USERPROFILE%\\.gradle",
        command: None,
        find: Paths(&[".gradle/caches", ".gradle/wrapper/dists", ".gradle/daemon", ".gradle/native"]),
    },
    RuleDef {
        id: "nuget-maven",
        level: Safe,
        title: ["แคช NuGet / Maven", "NuGet & Maven caches"],
        desc: ["แพ็กเกจ .NET และ Java ที่ดาวน์โหลดไว้ โหลดใหม่ตอน build", ".NET and Java packages. Downloaded again on the next build."],
        hint: "%USERPROFILE%\\.nuget\\packages",
        command: Some("dotnet nuget locals all --clear"),
        find: Paths(&[".nuget/packages", "AppData/Local/NuGet/v3-cache", "AppData/Local/NuGet/Cache", ".m2/repository"]),
    },
    RuleDef {
        id: "mobile-build",
        level: Safe,
        title: ["ไฟล์ build ของ Flutter / Android", "Flutter & Android build output"],
        desc: [
            "โฟลเดอร์ build, .dart_tool, .gradle และ .cxx ในโปรเจค สร้างใหม่ตอน build",
            "build, .dart_tool, .gradle and .cxx folders in projects. Rebuilt on the next build.",
        ],
        hint: "*\\build, .dart_tool",
        command: Some("flutter clean"),
        find: Pattern,
    },
    RuleDef {
        id: "python",
        level: Safe,
        title: ["แคช pip / uv / Poetry", "pip, uv & Poetry caches"],
        desc: ["แพ็กเกจ Python ที่ดาวน์โหลดไว้ ไม่กระทบ virtualenv", "Downloaded Python packages. Existing virtualenvs keep working."],
        hint: "%LOCALAPPDATA%\\pip\\Cache",
        command: Some("uv cache clean; pip cache purge"),
        find: Paths(&["AppData/Local/pip/Cache", "AppData/Local/uv/cache", "AppData/Local/pypoetry/Cache/cache"]),
    },
    RuleDef {
        id: "browsers",
        level: Safe,
        title: ["เบราว์เซอร์ทดสอบ (Puppeteer / Playwright)", "Puppeteer & Playwright browsers"],
        desc: ["Chrome/Firefox ที่เครื่องมือทดสอบดาวน์โหลดมา", "Browsers downloaded by test tools."],
        hint: "%LOCALAPPDATA%\\ms-playwright",
        command: Some("npx playwright install"),
        find: Paths(&[".cache/puppeteer", "AppData/Local/ms-playwright"]),
    },
    RuleDef {
        id: "app-web-caches",
        level: Safe,
        title: ["แคชของเบราว์เซอร์และแอป Electron", "Browser & Electron app caches"],
        desc: [
            "แคชหน้าเว็บ โค้ด และกราฟิกของ Chrome, Edge, VS Code, Discord, Teams ฯลฯ ไม่กระทบรหัสผ่าน ประวัติ หรือการตั้งค่า",
            "Web, code and GPU caches of Chrome, Edge, VS Code, Discord, Teams and others. Passwords, history and settings are untouched.",
        ],
        hint: "%LOCALAPPDATA%\\…\\User Data\\*\\Cache",
        command: None,
        find: Paths(&[
            "AppData/Local/Google/Chrome/User Data/*/Cache",
            "AppData/Local/Google/Chrome/User Data/*/Code Cache",
            "AppData/Local/Google/Chrome/User Data/*/GPUCache",
            "AppData/Local/Google/Chrome/User Data/*/Service Worker/CacheStorage",
            "AppData/Local/Microsoft/Edge/User Data/*/Cache",
            "AppData/Local/Microsoft/Edge/User Data/*/Code Cache",
            "AppData/Local/Microsoft/Edge/User Data/*/GPUCache",
            "AppData/Local/Microsoft/Edge/User Data/*/Service Worker/CacheStorage",
            "AppData/Local/BraveSoftware/Brave-Browser/User Data/*/Cache",
            "AppData/Local/BraveSoftware/Brave-Browser/User Data/*/Code Cache",
            "AppData/Roaming/*/Cache",
            "AppData/Roaming/*/Code Cache",
            "AppData/Roaming/*/GPUCache",
            "AppData/Roaming/*/CachedData",
            "AppData/Roaming/*/CachedExtensionVSIXs",
            "AppData/Roaming/*/Service Worker/CacheStorage",
        ]),
    },
    RuleDef {
        id: "shader-caches",
        level: Safe,
        title: ["แคชกราฟิก (shader)", "Graphics shader caches"],
        desc: [
            "แคช shader ของ DirectX และการ์ดจอ สร้างใหม่เองตอนเปิดเกมหรือแอป",
            "DirectX and GPU driver shader caches. Rebuilt when games and apps run.",
        ],
        hint: "%LOCALAPPDATA%\\D3DSCache, NVIDIA\\DXCache",
        command: None,
        find: Paths(&[
            "AppData/Local/D3DSCache",
            "AppData/Local/NVIDIA/DXCache",
            "AppData/Local/NVIDIA/GLCache",
            "AppData/Local/AMD/DxCache",
            "AppData/Local/AMD/GLCache",
        ]),
    },
    RuleDef {
        id: "user-temp",
        level: Safe,
        title: ["ไฟล์ชั่วคราว (Temp)", "Temporary files"],
        desc: [
            "ไฟล์ใน Temp ที่เก่ากว่า 3 วัน โปรแกรมที่ติดตั้งหรือทำงานเสร็จแล้วทิ้งไว้",
            "Files in Temp older than 3 days, left behind by installers and apps.",
        ],
        hint: "%LOCALAPPDATA%\\Temp",
        command: None,
        find: UserTemp,
    },
    RuleDef {
        id: "crash-reports",
        level: Safe,
        title: ["รายงานข้อขัดข้องและ crash dump", "Crash reports & dumps"],
        desc: [
            "ไฟล์ที่ Windows และแอปบันทึกไว้ตอนโปรแกรมค้างหรือปิดเอง ไม่จำเป็นต่อการใช้งาน",
            "Files Windows and apps saved when something crashed. Not needed to run anything.",
        ],
        hint: "%LOCALAPPDATA%\\CrashDumps, WER",
        command: None,
        find: Paths(&[
            "AppData/Local/CrashDumps",
            "AppData/Local/Microsoft/Windows/WER",
            "/ProgramData/Microsoft/Windows/WER/ReportArchive",
            "/ProgramData/Microsoft/Windows/WER/ReportQueue",
        ]),
    },
    RuleDef {
        id: "app-logs",
        level: Safe,
        title: ["ไฟล์ล็อกของ VS Code / Cursor", "VS Code & Cursor logs"],
        desc: ["ล็อกที่ editor เขียนทิ้งไว้", "Log files written by the editors."],
        hint: "%APPDATA%\\Code\\logs",
        command: None,
        find: Paths(&["AppData/Roaming/Code/logs", "AppData/Roaming/Cursor/logs"]),
    },
    // ─── Review ───
    RuleDef {
        id: "recycle-bin",
        level: Review,
        title: ["ถังรีไซเคิล", "Recycle Bin"],
        desc: [
            "ไฟล์ที่ลบไปแล้วยังกินพื้นที่อยู่จนกว่าจะล้างถัง ล้างแล้วกู้คืนไม่ได้",
            "Deleted files still use space until the bin is emptied. Emptying cannot be undone.",
        ],
        hint: "$Recycle.Bin",
        command: Some("Clear-RecycleBin -Force"),
        find: RecycleBin,
    },
    RuleDef {
        id: "android",
        level: Review,
        title: ["Android emulator และ system image", "Android emulators & system images"],
        desc: [
            "เครื่องจำลองและอิมเมจระบบ Android ถ้าจะใช้อีกต้องดาวน์โหลดใหม่ผ่าน Android Studio",
            "Emulator disks and system images. Download again from Android Studio if you need them.",
        ],
        hint: "%USERPROFILE%\\.android\\avd",
        command: None,
        find: Paths(&["AppData/Local/Android/Sdk/system-images", ".android/avd", ".android/cache"]),
    },
    RuleDef {
        id: "ollama",
        level: Review,
        title: ["โมเดล AI ของ Ollama", "Ollama models"],
        desc: ["โมเดลภาษาที่ดาวน์โหลดไว้ ถ้าจะลบบางตัวให้ใช้ ollama rm", "Downloaded language models. To remove just one, use ollama rm."],
        hint: "%USERPROFILE%\\.ollama\\models",
        command: Some("ollama list   # then: ollama rm <model>"),
        find: Paths(&[".ollama/models"]),
    },
    RuleDef {
        id: "ai-models",
        level: Review,
        title: ["โมเดล AI อื่น ๆ (Hugging Face / LM Studio)", "Other AI models (Hugging Face, LM Studio)"],
        desc: ["โมเดลและ dataset ที่ดาวน์โหลดไว้ โหลดใหม่ได้แต่ไฟล์ใหญ่", "Downloaded models and datasets. Re-downloadable, but large."],
        hint: "%USERPROFILE%\\.cache\\huggingface",
        command: None,
        find: Paths(&[".cache/huggingface", ".lmstudio/models"]),
    },
    RuleDef {
        id: "docker",
        level: Review,
        title: ["ดิสก์ของ Docker", "Docker disk image"],
        desc: [
            "เก็บ image, container และ volume ทั้งหมดของ Docker แนะนำใช้ docker system prune แทนการลบไฟล์",
            "Holds all Docker images, containers and volumes. Prefer docker system prune over deleting it.",
        ],
        hint: "%LOCALAPPDATA%\\Docker\\wsl",
        command: Some("docker system prune -a"),
        find: Paths(&["AppData/Local/Docker/wsl"]),
    },
    RuleDef {
        id: "idle-deps",
        level: Review,
        title: ["node_modules ในโปรเจคที่ไม่ได้แตะ", "node_modules in idle projects"],
        desc: [
            "โปรเจคที่ไม่มีไฟล์ถูกแก้เกิน 30 วัน ลบได้แล้วสั่ง npm install ใหม่เมื่อกลับมาทำ",
            "Projects with no edits for 30+ days. Run npm install again when you come back.",
        ],
        hint: "*\\node_modules",
        command: None,
        find: Pattern,
    },
    RuleDef {
        id: "idle-venv",
        level: Review,
        title: ["Python virtualenv ในโปรเจคที่ไม่ได้แตะ", "Python virtualenvs in idle projects"],
        desc: [
            "โฟลเดอร์ .venv / venv ของโปรเจคที่ไม่ได้แก้เกิน 30 วัน สร้างใหม่ได้จาก requirements",
            ".venv / venv folders in projects idle for 30+ days. Recreate from requirements.",
        ],
        hint: "*\\.venv",
        command: None,
        find: Pattern,
    },
    RuleDef {
        id: "worktrees",
        level: Review,
        title: ["git worktree ที่ Claude Code สร้างไว้", "Claude Code worktrees"],
        desc: ["สำเนาโปรเจคที่ใช้ทำงานคู่ขนาน ตรวจว่า merge/push แล้วก่อนลบ", "Parallel working copies. Make sure their work is merged or pushed first."],
        hint: "*\\.claude\\worktrees",
        command: Some("git worktree prune"),
        find: Pattern,
    },
    RuleDef {
        id: "installers",
        level: Review,
        title: ["ไฟล์ติดตั้งใน Downloads", "Installers in Downloads"],
        desc: ["ไฟล์ .exe .msi .iso ที่ติดตั้งเสร็จแล้วส่วนใหญ่ไม่ต้องเก็บ", ".exe, .msi and .iso files. Usually not needed once installed."],
        hint: "Downloads\\*.exe, *.msi",
        command: None,
        find: Installers,
    },
    RuleDef {
        id: "old-downloads",
        level: Review,
        title: ["ไฟล์ใหญ่เก่า ๆ ใน Downloads", "Large old downloads"],
        desc: ["ไฟล์เกิน 100 MB ที่ไม่ได้แตะเกิน 90 วัน", "Files over 100 MB untouched for 90+ days."],
        hint: "Downloads",
        command: None,
        find: OldDownloads,
    },
    RuleDef {
        id: "dot-cache",
        level: Review,
        title: ["แคชอื่น ๆ ใน .cache", "Other caches in .cache"],
        desc: ["แคชของเครื่องมือ command line ส่วนใหญ่ลบได้", "Command-line tool caches. Mostly disposable."],
        hint: "%USERPROFILE%\\.cache",
        command: None,
        find: ChildrenOf(".cache", &[]),
    },
];

const JS_BUILD: &[&str] = &[".next", ".turbo", ".nuxt", ".svelte-kit", ".parcel-cache", ".angular"];
/// Folders that do not count as "recent work" when deciding if a project is idle.
const NOT_ACTIVITY: &[&str] = &[
    "node_modules", ".git", ".next", ".turbo", ".nuxt", "target", ".venv", "venv", "dist", "build", ".cache",
    "Pods", ".dart_tool", ".gradle", ".build", ".cxx",
];
const INSTALLER_EXT: &[&str] = &["dmg", "pkg", "iso", "xip", "mpkg", "msi", "exe", "msix", "appx"];

#[derive(Serialize, Clone, Debug)]
pub struct Item {
    /// Tree node id, or `VIRTUAL + n` for things outside the scanned tree.
    pub id: u32,
    pub path: String,
    /// Friendlier name than the path, when we know one (e.g. "iPhone 17 · iOS 26.3").
    pub label: Option<String>,
    pub size: u64,
    pub files: u64,
    pub newest: i64,
    /// Last time an app or runtime was used (unix seconds, 0 = unknown).
    pub last_used: Option<i64>,
    /// Simulators that depend on a runtime.
    pub sims: Option<u32>,
    pub action: Action,
}

/// How an item is removed.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Action {
    /// Trash or delete as the user chose.
    Remove,
    /// Owned by another user (root): deleted with the admin password.
    Admin,
    /// Apps always go to the Trash through Finder, which asks for a password if needed.
    Finder,
    /// Run a tool that knows how to remove it.
    Command { argv: Vec<String> },
    /// Empty the Windows Recycle Bin.
    EmptyRecycleBin,
}

/// Ids at or above this are not tree nodes.
pub const VIRTUAL: u32 = 0xF000_0000;

#[derive(Serialize, Clone, Debug)]
pub struct Recommendation {
    pub id: &'static str,
    pub level: Level,
    pub title_th: &'static str,
    pub title_en: &'static str,
    pub desc_th: &'static str,
    pub desc_en: &'static str,
    pub hint: &'static str,
    pub command: Option<&'static str>,
    pub size: u64,
    pub items: Vec<Item>,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Last time anything in the project changed, ignoring generated folders.
fn activity(tree: &Tree, project: u32) -> i64 {
    let n = tree.node(project);
    let mut t = n.own_newest;
    for &c in &n.children {
        let cn = tree.node(c);
        if cn.is_removed() || NOT_ACTIVITY.contains(&&*cn.name) {
            continue;
        }
        t = t.max(cn.newest);
    }
    t
}

/// Walk project folders (outside ~/Library) once and collect pattern hits.
fn pattern_scan(tree: &Tree) -> HashMap<&'static str, Vec<u32>> {
    let mut hits: HashMap<&'static str, Vec<u32>> = HashMap::new();
    let idle_before = now() - IDLE_DAYS * 86400;
    let at_home = tree.root == tree.home || tree.home_id != 0;
    let mut stack: Vec<u32> = vec![tree.home_id];
    while let Some(id) = stack.pop() {
        let node = tree.node(id);
        if id != tree.home_id {
            let name = &*node.name;
            let parent = tree.node(node.parent);
            let pflags = parent.flags;
            use crate::tree::flag;
            if name == "worktrees" && &*parent.name == ".claude" {
                for &c in &node.children {
                    if !tree.node(c).is_file() {
                        hits.entry("worktrees").or_default().push(c);
                    }
                }
                continue;
            }
            if JS_BUILD.contains(&name) && pflags & flag::PKG_JSON != 0 {
                hits.entry("js-build").or_default().push(id);
                continue;
            }
            if name == "target" && pflags & flag::CARGO != 0 {
                hits.entry("rust-target").or_default().push(id);
                continue;
            }
            let mobile_build = ((name == "build" || name == ".dart_tool") && pflags & flag::PUBSPEC != 0)
                || (matches!(name, "build" | ".gradle" | ".cxx") && pflags & flag::GRADLE != 0)
                || (name == ".build" && pflags & flag::SWIFTPM != 0);
            if mobile_build {
                hits.entry("mobile-build").or_default().push(id);
                continue;
            }
            let deps = (name == "node_modules" && pflags & flag::PKG_JSON != 0)
                || (name == "Pods" && pflags & flag::PODFILE != 0);
            if deps || name == "node_modules" {
                if deps && activity(tree, node.parent) < idle_before {
                    hits.entry("idle-deps").or_default().push(id);
                }
                continue;
            }
            if node.flags & flag::PYVENV != 0 {
                if activity(tree, node.parent) < idle_before {
                    hits.entry("idle-venv").or_default().push(id);
                }
                continue;
            }
            if name == ".git" {
                continue;
            }
            if at_home && node.parent == tree.home_id && matches!(name, "Library" | ".Trash") {
                continue;
            }
        }
        for &c in &node.children {
            let cn = tree.node(c);
            if !cn.is_file() && !cn.is_removed() {
                stack.push(c);
            }
        }
    }
    hits
}

fn downloads(tree: &Tree, installers: bool) -> Vec<u32> {
    let Some(dl) = tree.lookup("Downloads") else { return vec![] };
    let old_before = now() - OLD_DOWNLOAD_DAYS * 86400;
    let mut out = Vec::new();
    let mut stack = vec![(dl, 0u8)];
    while let Some((id, depth)) = stack.pop() {
        for &c in &tree.node(id).children {
            let n = tree.node(c);
            if n.is_removed() {
                continue;
            }
            if !n.is_file() {
                // Look one level into sub-folders too.
                if depth < 1 {
                    stack.push((c, depth + 1));
                }
                continue;
            }
            let ext = n.name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
            let is_installer = INSTALLER_EXT.contains(&ext.as_str());
            let wanted = if installers {
                is_installer
            } else {
                !is_installer && n.size >= OLD_DOWNLOAD_MIN && n.newest < old_before
            };
            if wanted {
                out.push(c);
            }
        }
    }
    out
}

/// Expand a home-relative path whose segments may be `*`.
fn glob(tree: &Tree, pattern: &str) -> Vec<u32> {
    if let Some(below_root) = pattern.strip_prefix('/') {
        // Relative to `/` on macOS, to the system drive on Windows.
        let path = crate::scan::system_root().join(below_root);
        return tree.lookup_abs(&path).into_iter().collect();
    }
    let mut cur = vec![tree.home_id];
    for seg in pattern.split('/').filter(|s| !s.is_empty()) {
        let mut next = Vec::new();
        for id in cur {
            for &c in &tree.node(id).children {
                let n = tree.node(c);
                if n.is_removed() {
                    continue;
                }
                let ok = if seg == "*" {
                    !n.is_file()
                } else if let Some(suffix) = seg.strip_prefix('*') {
                    n.name.ends_with(suffix)
                } else {
                    &*n.name == seg
                };
                if ok {
                    next.push(c);
                }
            }
        }
        cur = next;
    }
    cur
}

/// Read one string value from a small XML property list.
fn plist_string(xml: &str, key: &str) -> Option<String> {
    let at = xml.find(&format!("<key>{key}</key>"))?;
    let rest = &xml[at..];
    let start = rest.find("<string>")? + "<string>".len();
    let end = rest[start..].find("</string>")?;
    Some(rest[start..start + end].to_string())
}

fn simulators(tree: &Tree, labels: &mut HashMap<u32, String>) -> Vec<u32> {
    let Some(dir) = tree.lookup("Library/Developer/CoreSimulator/Devices") else { return vec![] };
    let mut out = Vec::new();
    for &c in &tree.node(dir).children {
        let n = tree.node(c);
        if n.is_file() || n.is_removed() {
            continue;
        }
        let plist = std::fs::read_to_string(tree.path(c).join("device.plist")).unwrap_or_default();
        if let Some(name) = plist_string(&plist, "name") {
            let runtime = plist_string(&plist, "runtime")
                .and_then(|r| r.rsplit('.').next().map(|v| v.replacen('-', " ", 1).replace('-', ".")))
                .unwrap_or_default();
            labels.insert(c, if runtime.is_empty() { name } else { format!("{name} · {runtime}") });
        }
        out.push(c);
    }
    out
}

fn brew_old(tree: &Tree) -> Vec<u32> {
    let prefix = std::path::Path::new("/opt/homebrew");
    let Some(cellar) = tree.lookup_abs(&prefix.join("Cellar")) else { return vec![] };
    let mut out = Vec::new();
    for &f in &tree.node(cellar).children {
        let formula = &*tree.node(f).name;
        // opt/<formula> points at the version in use; everything else is old.
        let Ok(target) = std::fs::read_link(prefix.join("opt").join(formula)) else { continue };
        let Some(current) = target.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
        for &v in &tree.node(f).children {
            let n = tree.node(v);
            if !n.is_file() && *n.name != *current {
                out.push(v);
            }
        }
    }
    out
}

const UNUSED_APP_DAYS: i64 = 90;
const UNUSED_APP_MIN: u64 = 20 * 1000 * 1000;

fn unused_apps(tree: &Tree, sys: &SysInfo, last_used: &mut HashMap<u32, i64>) -> Vec<u32> {
    let before = now() - UNUSED_APP_DAYS * 86400;
    let mut out = Vec::new();
    for (app, &t) in &sys.app_last_used {
        let Some(id) = tree.lookup_abs(app) else { continue };
        if t < before && tree.node(id).size >= UNUSED_APP_MIN {
            last_used.insert(id, t);
            out.push(id);
        }
    }
    out
}

const TEMP_DAYS: i64 = 3;

#[cfg(windows)]
fn user_temp(tree: &Tree) -> Vec<u32> {
    let before = now() - TEMP_DAYS * 86400;
    match tree.lookup("AppData/Local/Temp") {
        Some(t) => tree.node(t).children.iter().copied().filter(|&c| tree.node(c).newest < before).collect(),
        None => vec![],
    }
}

#[cfg(not(windows))]
fn user_temp(tree: &Tree) -> Vec<u32> {
    let Some(dir) = crate::scan::user_var_dir() else { return vec![] };
    let before = now() - TEMP_DAYS * 86400;
    let mut out = Vec::new();
    if let Some(t) = tree.lookup_abs(&dir.join("T")) {
        out.extend(tree.node(t).children.iter().copied().filter(|&c| tree.node(c).newest < before));
    }
    if let Some(c) = tree.lookup_abs(&dir.join("C")) {
        out.extend(tree.node(c).children.iter().copied().filter(|&k| !tree.node(k).name.starts_with("com.apple.")));
    }
    out
}

/// Windows: anything outside the user's profile needs elevation to delete.
#[cfg(windows)]
fn owned_by_me(tree: &Tree, path: &std::path::Path) -> bool {
    path.starts_with(&tree.home)
}

#[cfg(unix)]
fn owned_by_me(_tree: &Tree, path: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let uid = unsafe { libc::getuid() };
    std::fs::symlink_metadata(path).map(|m| m.uid() == uid).unwrap_or(true)
}

pub fn recommend(tree: &Tree, sys: &SysInfo) -> Vec<Recommendation> {
    let hits = pattern_scan(tree);
    let mut claimed: HashSet<u32> = HashSet::new();
    let mut claimed_below: HashSet<u32> = HashSet::new();
    let mut out = Vec::new();
    let mut labels: HashMap<u32, String> = HashMap::new();
    let mut last_used: HashMap<u32, i64> = HashMap::new();
    // Never suggest the folder this program is running from (e.g. a Rust
    // project's target/ holding the freshly built app).
    let me = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
    let holds_me = |id: u32| me.as_ref().is_some_and(|m| m.starts_with(tree.path(id)));

    for def in RULES {
        let candidates: Vec<u32> = match &def.find {
            Paths(ps) => ps.iter().flat_map(|p| glob(tree, p)).collect(),
            ChildrenOf(p, skip) => match tree.lookup(p) {
                Some(dir) => tree
                    .node(dir)
                    .children
                    .iter()
                    .copied()
                    .filter(|&c| !skip.iter().any(|s| tree.node(c).name.starts_with(s)))
                    .collect(),
                None => vec![],
            },
            Pattern => hits.get(def.id).cloned().unwrap_or_default(),
            Installers => downloads(tree, true),
            OldDownloads => downloads(tree, false),
            Simulators => simulators(tree, &mut labels),
            BrewOld => brew_old(tree),
            UnusedApps => unused_apps(tree, sys, &mut last_used),
            UserTemp => user_temp(tree),
            Runtimes | RecycleBin => vec![],
        };

        let mut items = Vec::new();
        if matches!(def.find, RecycleBin) {
            if let Some((size, count)) = crate::disk::recycle_bin() {
                items.push(Item {
                    id: VIRTUAL + 0x0800_0000,
                    path: "$Recycle.Bin".into(),
                    label: None,
                    size,
                    files: count,
                    newest: 0,
                    last_used: None,
                    sims: None,
                    action: Action::EmptyRecycleBin,
                });
            }
        }
        if matches!(def.find, Runtimes) {
            for (i, r) in sys.runtimes.iter().enumerate() {
                items.push(Item {
                    id: VIRTUAL + i as u32,
                    path: r.path.display().to_string(),
                    label: Some(r.label.clone()),
                    size: r.size,
                    files: 0,
                    newest: r.last_used,
                    last_used: Some(r.last_used),
                    sims: Some(r.sims),
                    action: Action::Command {
                        argv: ["/usr/bin/xcrun", "simctl", "runtime", "delete", &r.id].map(String::from).to_vec(),
                    },
                });
            }
        }
        let mut queue = candidates;
        while let Some(c) = queue.pop() {
            let n = tree.node(c);
            if n.is_removed() || n.size == 0 || holds_me(c) {
                continue;
            }
            if claimed.contains(&c) || tree.ancestors(c).any(|a| claimed.contains(&a)) {
                continue;
            }
            if claimed_below.contains(&c) {
                // Part of this folder already belongs to a more specific rule.
                queue.extend(n.children.iter().copied());
                continue;
            }
            claimed.insert(c);
            for a in tree.ancestors(c) {
                if !claimed_below.insert(a) {
                    break;
                }
            }
            let path = tree.path(c);
            let action = if matches!(def.find, UnusedApps) {
                Action::Finder
            } else if owned_by_me(tree, &path) {
                Action::Remove
            } else {
                Action::Admin
            };
            let label = labels.get(&c).cloned().or_else(|| {
                matches!(def.find, UnusedApps).then(|| n.name.trim_end_matches(".app").to_string())
            });
            items.push(Item {
                id: c,
                path: tree.display(c),
                label,
                size: n.size,
                files: n.files,
                newest: n.newest,
                last_used: last_used.get(&c).copied(),
                sims: None,
                action,
            });
        }
        let size: u64 = items.iter().map(|i| i.size).sum();
        if size < MIN_RULE_SIZE {
            continue;
        }
        items.sort_by(|a, b| b.size.cmp(&a.size));
        out.push(Recommendation {
            id: def.id,
            level: def.level,
            title_th: def.title[0],
            title_en: def.title[1],
            desc_th: def.desc[0],
            desc_en: def.desc[1],
            hint: def.hint,
            command: def.command,
            size,
            items,
        });
    }
    // Biggest first within each level, safe before review.
    out.sort_by(|a, b| {
        (a.level == Review).cmp(&(b.level == Review)).then(b.size.cmp(&a.size))
    });
    out
}

/// node id -> (recommendation index, level) for every item.
pub fn item_index(recs: &[Recommendation]) -> HashMap<u32, (usize, Level)> {
    let mut m = HashMap::new();
    for (i, r) in recs.iter().enumerate() {
        for it in &r.items {
            m.insert(it.id, (i, r.level));
        }
    }
    m
}

/// For every folder holding items: (safe bytes, review bytes) inside it, so
/// the treemap can show how much of a folder can be reclaimed.
pub fn containers(tree: &Tree, recs: &[Recommendation]) -> HashMap<u32, [u64; 2]> {
    let mut m: HashMap<u32, [u64; 2]> = HashMap::new();
    for r in recs {
        let k = if r.level == Safe { 0 } else { 1 };
        for it in r.items.iter().filter(|it| it.id < VIRTUAL) {
            for a in tree.ancestors(it.id) {
                m.entry(a).or_default()[k] += it.size;
            }
        }
    }
    m
}
