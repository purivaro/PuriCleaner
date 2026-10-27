//! What is using RAM right now, grouped by app, and a polite way to quit an
//! app the user no longer needs.
//!
//! No "free RAM" tricks: purging the system cache only makes the machine
//! reload it from disk. The only RAM worth getting back is what open apps
//! hold, and the right way to get it is to quit them normally (the app may
//! ask to save first).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use ::sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Pressure {
    Normal,
    Warn,
    Critical,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A normal app with windows: may be quit.
    App,
    /// Menu-bar agents, helpers and other background programs of the user.
    Background,
    /// Part of the operating system: never offered.
    System,
}

#[derive(Serialize, Clone, Debug)]
pub struct AppUse {
    /// Stable key for this group (bundle path or exe path / name).
    pub key: String,
    pub name: String,
    pub path: Option<String>,
    pub kind: Kind,
    pub memory: u64,
    /// Percent of one core, summed over the group.
    pub cpu: f32,
    pub processes: usize,
    /// Seconds since the oldest process of the group started.
    pub running_for: u64,
    pub can_quit: bool,
    #[serde(skip)]
    roots: Vec<Pid>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Snapshot {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub pressure: Pressure,
    pub apps: Vec<AppUse>,
}

#[derive(Clone, Default)]
struct BundleInfo {
    name: String,
    agent: bool,
}

pub struct Memory {
    sys: System,
    me: Option<Pid>,
    bundles: HashMap<PathBuf, BundleInfo>,
    last: Vec<AppUse>,
    last_refresh: Option<Instant>,
}

/// Processes that keep the desktop itself running.
const SYSTEM_NAMES: &[&str] = &[
    // macOS
    "kernel_task", "launchd", "WindowServer", "loginwindow", "Dock", "Finder", "SystemUIServer",
    "ControlCenter", "NotificationCenter", "Spotlight", "coreaudiod", "mds", "mds_stores", "mdworker_shared",
    // Windows
    "System", "Registry", "smss.exe", "csrss.exe", "wininit.exe", "winlogon.exe", "services.exe", "lsass.exe",
    "svchost.exe", "dwm.exe", "explorer.exe", "sihost.exe", "fontdrvhost.exe", "ctfmon.exe", "RuntimeBroker.exe",
    "SearchHost.exe", "StartMenuExperienceHost.exe", "ShellExperienceHost.exe", "TextInputHost.exe",
    "MsMpEng.exe", "SecurityHealthService.exe", "audiodg.exe", "Memory Compression", "spoolsv.exe",
];

impl Default for Memory {
    fn default() -> Self {
        Memory { sys: System::new(), me: ::sysinfo::get_current_pid().ok(), bundles: HashMap::new(), last: Vec::new(), last_refresh: None }
    }
}

/// The outermost `.app` bundle in a path ("…/Edge.app/Contents/Frameworks/Helper.app/…" → "…/Edge.app").
pub(crate) fn outer_bundle(exe: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in exe.components() {
        out.push(c);
        if out.extension().is_some_and(|e| e == "app") {
            return Some(out);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn read_bundle(path: &Path) -> BundleInfo {
    let fallback = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let out = std::process::Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(path.join("Contents/Info.plist"))
        .output();
    let Ok(out) = out else { return BundleInfo { name: fallback, ..Default::default() } };
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    let flag = |k: &str| match &v[k] {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::String(s) => s == "1" || s.eq_ignore_ascii_case("true"),
        serde_json::Value::Number(n) => n.as_i64() == Some(1),
        _ => false,
    };
    BundleInfo {
        // The name Finder shows ("Visual Studio Code", not its short CFBundleName "Code").
        name: fallback,
        agent: flag("LSUIElement") || flag("LSBackgroundOnly"),
    }
}

#[cfg(not(target_os = "macos"))]
fn read_bundle(path: &Path) -> BundleInfo {
    BundleInfo { name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), ..Default::default() }
}

/// The kernel's own "memory free" percentage — what `memory_pressure`
/// prints. sysinfo's figure on macOS leaves out reclaimable memory.
#[cfg(target_os = "macos")]
fn free_percent() -> Option<u64> {
    let mut level: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>();
    let name = c"kern.memorystatus_level";
    let ok = unsafe { libc::sysctlbyname(name.as_ptr(), (&mut level as *mut libc::c_int).cast(), &mut len, std::ptr::null_mut(), 0) } == 0;
    (ok && (0..=100).contains(&level)).then_some(level as u64)
}

#[cfg(not(target_os = "macos"))]
fn free_percent() -> Option<u64> {
    None
}

#[cfg(target_os = "macos")]
fn pressure(used_pct: f64, swap_pct: f64) -> Pressure {
    // The kernel's own verdict: 1 normal, 2 warning, 4 critical.
    let mut level: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>();
    let name = c"kern.memorystatus_vm_pressure_level";
    let ok = unsafe { libc::sysctlbyname(name.as_ptr(), (&mut level as *mut libc::c_int).cast(), &mut len, std::ptr::null_mut(), 0) } == 0;
    match (ok, level) {
        (true, 4) => Pressure::Critical,
        (true, 2) => Pressure::Warn,
        (true, _) => Pressure::Normal,
        _ => fallback_pressure(used_pct, swap_pct),
    }
}

#[cfg(not(target_os = "macos"))]
fn pressure(used_pct: f64, swap_pct: f64) -> Pressure {
    fallback_pressure(used_pct, swap_pct)
}

/// Windows has no single pressure figure: judge by how full RAM and the page file are.
fn fallback_pressure(used_pct: f64, swap_pct: f64) -> Pressure {
    if used_pct > 92.0 || (used_pct > 85.0 && swap_pct > 70.0) {
        Pressure::Critical
    } else if used_pct > 80.0 {
        Pressure::Warn
    } else {
        Pressure::Normal
    }
}

impl Memory {
    pub fn snapshot(&mut self) -> Snapshot {
        // CPU use needs two samples; the first call gives a quick baseline.
        let kind = ProcessRefreshKind::nothing()
            .with_memory()
            .with_cpu()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_user(UpdateKind::OnlyIfNotSet);
        if self.last_refresh.is_none() {
            self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
            std::thread::sleep(::sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(250)));
        }
        self.sys.refresh_memory();
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        self.last_refresh = Some(Instant::now());

        let my_user = self.me.and_then(|p| self.sys.process(p)).and_then(|p| p.user_id()).cloned();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

        struct Acc {
            name: String,
            path: Option<String>,
            kind: Kind,
            memory: u64,
            cpu: f32,
            pids: Vec<Pid>,
            started: u64,
            /// The bundle's own program (Contents/MacOS/…) is running, not just helpers.
            has_main: bool,
        }
        let mut groups: HashMap<String, Acc> = HashMap::new();
        for (pid, p) in self.sys.processes() {
            if p.thread_kind().is_some() {
                continue; // Linux threads; never on macOS / Windows
            }
            let name = p.name().to_string_lossy().into_owned();
            let exe = p.exe().map(Path::to_path_buf);
            let mine = my_user.is_some() && p.user_id() == my_user.as_ref();
            let mut is_main = cfg!(not(target_os = "macos"));
            let (key, label, path, mut kind) = match exe.as_deref().and_then(outer_bundle) {
                Some(bundle) => {
                    let info = self.bundles.entry(bundle.clone()).or_insert_with(|| read_bundle(&bundle)).clone();
                    let in_system = bundle.starts_with("/System") || bundle.starts_with("/Library/Apple");
                    is_main = exe.as_deref().is_some_and(|e| e.starts_with(bundle.join("Contents/MacOS")));
                    // Apps living inside a framework (Python.app) run scripts, not windows.
                    let in_framework = bundle.to_string_lossy().contains(".framework/");
                    let kind = if in_system {
                        Kind::System
                    } else if info.agent || in_framework {
                        Kind::Background
                    } else {
                        Kind::App
                    };
                    (bundle.display().to_string(), info.name, Some(bundle.display().to_string()), kind)
                }
                None => {
                    let path = exe.as_ref().map(|e| e.display().to_string());
                    let in_os = path.as_deref().is_some_and(|p| {
                        let l = p.to_ascii_lowercase();
                        l.starts_with("/usr/") || l.starts_with("/system/") || l.starts_with("/sbin") || l.contains(":\\windows\\")
                    });
                    let kind = if !mine || in_os { Kind::System } else if cfg!(windows) { Kind::App } else { Kind::Background };
                    (path.clone().unwrap_or_else(|| name.clone()), name.clone(), path, kind)
                }
            };
            if SYSTEM_NAMES.iter().any(|s| s.eq_ignore_ascii_case(&name)) || !mine {
                kind = Kind::System;
            }
            let g = groups
                .entry(key)
                .or_insert_with(|| Acc { name: label, path, kind, memory: 0, cpu: 0.0, pids: vec![], started: u64::MAX, has_main: false });
            g.has_main |= is_main;
            g.memory += p.memory();
            g.cpu += p.cpu_usage();
            g.pids.push(*pid);
            if p.start_time() > 0 {
                g.started = g.started.min(p.start_time());
            }
            // A group is as protected as its most protected member.
            if kind == Kind::System {
                g.kind = Kind::System;
            }
        }

        let mut apps: Vec<AppUse> = groups
            .into_iter()
            .map(|(key, mut g)| {
                // Only helpers of an app that is not open (e.g. a browser's
                // native-messaging host): nothing to quit.
                if g.kind == Kind::App && !g.has_main {
                    g.kind = Kind::Background;
                }
                // Roots: members whose parent is outside the group. Quitting
                // those lets the app close its own helpers.
                let set: std::collections::HashSet<Pid> = g.pids.iter().copied().collect();
                let roots: Vec<Pid> = g
                    .pids
                    .iter()
                    .copied()
                    .filter(|p| self.sys.process(*p).and_then(|x| x.parent()).is_none_or(|pp| !set.contains(&pp)))
                    .collect();
                let has_me = self.me.is_some_and(|m| set.contains(&m));
                AppUse {
                    can_quit: g.kind == Kind::App && !has_me,
                    key,
                    name: g.name,
                    path: g.path,
                    kind: g.kind,
                    memory: g.memory,
                    cpu: g.cpu,
                    processes: g.pids.len(),
                    running_for: now.saturating_sub(if g.started == u64::MAX { now } else { g.started }),
                    roots,
                }
            })
            .collect();
        apps.sort_by_key(|a| std::cmp::Reverse(a.memory));

        let total = self.sys.total_memory();
        let available = free_percent().map(|pct| total / 100 * pct).unwrap_or_else(|| self.sys.available_memory());
        let (swap_total, swap_used) = (self.sys.total_swap(), self.sys.used_swap());
        let used = total.saturating_sub(available);
        let pct = |a: u64, b: u64| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
        self.last = apps.clone();
        Snapshot {
            total,
            used,
            available,
            swap_total,
            swap_used,
            pressure: pressure(pct(used, total), pct(swap_used, swap_total)),
            apps,
        }
    }

    /// Ask an app to quit normally, as if the user chose Quit. It may show a
    /// "save changes?" dialog and stay open — that is intended.
    pub fn quit(&mut self, key: &str) -> Result<(), String> {
        let app = self.last.iter().find(|a| a.key == key).cloned().ok_or("this app is no longer in the list")?;
        if !app.can_quit {
            return Err("this program is part of the system and is not closed by Puri Cleaner".into());
        }
        quit_roots(&app)
    }

    /// Memory the app's processes still hold (0 once it has quit).
    pub fn memory_of(&mut self, key: &str) -> u64 {
        let Some(app) = self.last.iter().find(|a| a.key == key) else { return 0 };
        let pids: Vec<Pid> = app.roots.clone();
        self.sys.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, ProcessRefreshKind::nothing().with_memory());
        pids.iter().filter_map(|p| self.sys.process(*p)).map(|p| p.memory()).sum()
    }
}

#[cfg(target_os = "macos")]
fn quit_roots(app: &AppUse) -> Result<(), String> {
    // NSRunningApplication.terminate is what the Dock's "Quit" does: the app
    // gets a normal quit request and can ask to save. No Automation permission needed.
    let pids = app.roots.iter().map(|p| p.as_u32().to_string()).collect::<Vec<_>>().join(",");
    let js = format!(
        "ObjC.import('AppKit'); var ok = false; [{pids}].forEach(function (p) {{ \
           var a = $.NSRunningApplication.runningApplicationWithProcessIdentifier(p); \
           if (a && !a.isNil()) {{ a.terminate; ok = true; }} }}); ok"
    );
    let out = std::process::Command::new("/usr/bin/osascript").args(["-l", "JavaScript", "-e", &js]).output().map_err(|e| e.to_string())?;
    if String::from_utf8_lossy(&out.stdout).trim() == "true" {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string().chars().take(200).collect::<String>() + " (no app window to quit)")
    }
}

#[cfg(windows)]
fn quit_roots(app: &AppUse) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // taskkill without /F sends WM_CLOSE: the app closes as if the user did it.
    let mut cmd = std::process::Command::new("taskkill");
    for p in &app.roots {
        cmd.args(["/PID", &p.as_u32().to_string()]);
    }
    cmd.arg("/T");
    let out = cmd.creation_flags(CREATE_NO_WINDOW).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn quit_roots(_app: &AppUse) -> Result<(), String> {
    Err("not supported on this system".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_outer_bundle() {
        let p = Path::new("/Applications/Microsoft Edge.app/Contents/Frameworks/Edge Framework.framework/Helpers/Edge Helper (Renderer).app/Contents/MacOS/Edge Helper (Renderer)");
        assert_eq!(outer_bundle(p), Some(PathBuf::from("/Applications/Microsoft Edge.app")));
        assert_eq!(outer_bundle(Path::new("/usr/bin/zsh")), None);
    }

    #[test]
    fn refuses_to_quit_system_background_or_unknown_entries() {
        // Fake entries with no real process behind them: the guard must stop
        // before any quit request is sent.
        let fake = |key: &str, kind: Kind, can_quit: bool| AppUse {
            key: key.into(),
            name: key.into(),
            path: None,
            kind,
            memory: 0,
            cpu: 0.0,
            processes: 1,
            running_for: 0,
            can_quit,
            roots: vec![],
        };
        let mut m = Memory::default();
        m.last = vec![fake("finder", Kind::System, false), fake("agent", Kind::Background, false)];
        assert!(m.quit("finder").unwrap_err().contains("system"));
        assert!(m.quit("agent").unwrap_err().contains("system"));
        assert!(m.quit("not-in-the-list").is_err());
    }

    #[test]
    fn system_processes_are_never_quittable() {
        let mut m = Memory::default();
        let s = m.snapshot();
        for a in &s.apps {
            if a.kind != Kind::App {
                assert!(!a.can_quit, "{} should not be quittable", a.name);
            }
            if ["Finder", "Dock", "WindowServer", "loginwindow", "kernel_task", "launchd"].contains(&a.name.as_str()) {
                assert_eq!(a.kind, Kind::System, "{} must be System", a.name);
            }
        }
    }

    #[test]
    fn snapshot_lists_this_process_as_not_quittable() {
        let mut m = Memory::default();
        let s = m.snapshot();
        assert!(s.total > 0 && s.used <= s.total);
        assert!(!s.apps.is_empty());
        assert!(s.apps.windows(2).all(|w| w[0].memory >= w[1].memory));
        // Never offer to quit ourselves.
        let me = ::sysinfo::get_current_pid().unwrap();
        for a in &s.apps {
            if a.roots.contains(&me) {
                assert!(!a.can_quit);
            }
        }
    }
}
