//! Facts the file tree alone cannot tell: installed simulator runtimes and
//! when each app was last opened. Gathered once after a scan.

use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::{fs, os::unix::fs::MetadataExt, process::Command};

#[cfg(target_os = "macos")]
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Runtime {
    pub id: String,
    pub label: String,
    pub size: u64,
    pub last_used: i64,
    pub path: PathBuf,
    /// Simulators (devices) that use this runtime.
    pub sims: u32,
}

#[derive(Clone, Debug, Default)]
pub struct SysInfo {
    pub runtimes: Vec<Runtime>,
    /// App bundle -> last time it was opened (unix seconds, 0 if unknown).
    pub app_last_used: HashMap<PathBuf, i64>,
    /// Running with administrator rights (Windows).
    pub elevated: bool,
    /// WinSxS analysis from DISM; only available when elevated.
    pub component_store: Option<ComponentStore>,
}

/// What `DISM /AnalyzeComponentStore` reports about C:\Windows\WinSxS.
#[derive(Clone, Debug, Default)]
pub struct ComponentStore {
    pub actual_size: u64,
    pub backups: u64,
    pub cache: u64,
    pub reclaimable_packages: u32,
    pub recommended: bool,
}

impl ComponentStore {
    /// Rough size `StartComponentCleanup` can free: temporary data always,
    /// plus superseded backups when DISM found packages to remove.
    pub fn estimate(&self) -> u64 {
        self.cache + if self.reclaimable_packages > 0 { self.backups } else { 0 }
    }
}

/// "3.21 GB" / "150.00 MB" / "0 bytes" -> bytes (DISM uses 1024-based units).
pub fn parse_dism_size(v: &str) -> Option<u64> {
    let mut it = v.split_whitespace();
    let n: f64 = it.next()?.replace(',', "").parse().ok()?;
    let mul = match it.next().unwrap_or("bytes").to_ascii_lowercase().as_str() {
        "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        "tb" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    Some((n * mul) as u64)
}

pub fn parse_dism(out: &str) -> ComponentStore {
    let mut c = ComponentStore::default();
    for line in out.lines() {
        let Some((k, v)) = line.split_once(" : ") else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "Actual Size of Component Store" => c.actual_size = parse_dism_size(v).unwrap_or(0),
            "Backups and Disabled Features" => c.backups = parse_dism_size(v).unwrap_or(0),
            "Cache and Temporary Data" => c.cache = parse_dism_size(v).unwrap_or(0),
            "Number of Reclaimable Packages" => c.reclaimable_packages = v.parse().unwrap_or(0),
            "Component Store Cleanup Recommended" => c.recommended = v.eq_ignore_ascii_case("yes"),
            _ => {}
        }
    }
    c
}

#[cfg(windows)]
pub fn is_elevated() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(target_os = "macos")]
/// Parse "2026-10-13T18:24:40Z" or "2026-10-15 09:20:58 +0000" (UTC) to unix seconds.
fn parse_time(s: &str) -> Option<i64> {
    let s = s.trim();
    let num = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    Some(days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se)
}

#[cfg(target_os = "macos")]
fn run_json(args: &[&str]) -> Option<Value> {
    let out = Command::new("/usr/bin/xcrun").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

#[cfg(target_os = "macos")]
fn runtimes() -> Vec<Runtime> {
    let Some(Value::Object(list)) = run_json(&["simctl", "runtime", "list", "-j"]) else { return vec![] };
    // runtimeIdentifier -> number of simulators using it
    let mut sims: HashMap<String, u32> = HashMap::new();
    if let Some(Value::Object(devs)) = run_json(&["simctl", "list", "devices", "-j"]).and_then(|v| v.get("devices").cloned()) {
        for (rt, list) in devs {
            sims.insert(rt, list.as_array().map(|a| a.len() as u32).unwrap_or(0));
        }
    }
    let mut out = Vec::new();
    for (id, r) in list {
        if r.get("deletable").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let rt_id = r.get("runtimeIdentifier").and_then(Value::as_str).unwrap_or("");
        let platform = rt_id.rsplit('.').next().and_then(|v| v.split('-').next()).unwrap_or("Simulator");
        let version = r.get("version").and_then(Value::as_str).unwrap_or("");
        let build = r.get("build").and_then(Value::as_str).unwrap_or("");
        out.push(Runtime {
            id: id.clone(),
            label: format!("{platform} {version} ({build})"),
            size: r.get("sizeBytes").and_then(Value::as_u64).unwrap_or(0),
            last_used: r.get("lastUsedAt").and_then(Value::as_str).and_then(parse_time).unwrap_or(0),
            path: PathBuf::from(r.get("path").and_then(Value::as_str).unwrap_or("")),
            sims: sims.get(rt_id).copied().unwrap_or(0),
        });
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.size));
    out
}

#[cfg(target_os = "macos")]
/// .app bundles in /Applications and one folder deeper.
fn app_bundles() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir("/Applications") else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "app") {
            out.push(p);
        } else if p.is_dir() && !p.is_symlink() {
            if let Ok(inner) = fs::read_dir(&p) {
                out.extend(inner.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "app")));
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
/// When the app's program last ran: the newer of Spotlight's "last used"
/// and the executable's access time (Spotlight misses apps not opened via Finder).
pub(crate) fn app_last_used(apps: &[PathBuf]) -> HashMap<PathBuf, i64> {
    let mut m: HashMap<PathBuf, i64> = HashMap::new();
    for app in apps {
        let mut t = 0i64;
        if let Ok(rd) = fs::read_dir(app.join("Contents/MacOS")) {
            for e in rd.flatten() {
                if let Ok(md) = e.metadata() {
                    t = t.max(md.atime());
                }
            }
        }
        m.insert(app.clone(), t);
    }
    if !apps.is_empty() {
        if let Ok(out) = Command::new("/usr/bin/mdls").args(["-name", "kMDItemLastUsedDate"]).args(apps).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = text.lines().filter(|l| l.contains("kMDItemLastUsedDate")).collect();
            if lines.len() == apps.len() {
                for (app, line) in apps.iter().zip(lines) {
                    if let Some(t) = line.split_once('=').and_then(|(_, v)| parse_time(v)) {
                        let e = m.entry(app.clone()).or_default();
                        *e = (*e).max(t);
                    }
                }
            }
        }
    }
    m
}

#[cfg(windows)]
pub fn gather(whole_mac: bool) -> SysInfo {
    let elevated = is_elevated();
    // DISM needs administrator rights and takes a minute or two.
    let component_store = (whole_mac && elevated).then(|| {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("Dism.exe")
            .args(["/English", "/Online", "/Cleanup-Image", "/AnalyzeComponentStore"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| parse_dism(&String::from_utf8_lossy(&o.stdout)))
    }).flatten();
    SysInfo { elevated, component_store, ..Default::default() }
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn gather(_whole_mac: bool) -> SysInfo {
    SysInfo::default()
}

#[cfg(target_os = "macos")]
pub fn gather(whole_mac: bool) -> SysInfo {
    if !whole_mac {
        return SysInfo::default();
    }
    let apps = app_bundles();
    SysInfo { runtimes: runtimes(), app_last_used: app_last_used(&apps), ..Default::default() }
}

#[cfg(test)]
mod dism_tests {
    use super::*;

    #[test]
    fn parses_dism_report() {
        let out = "Component Store (WinSxS) information:\r\n\r\nWindows Explorer Reported Size of Component Store : 10.52 GB\r\n\r\nActual Size of Component Store : 9.87 GB\r\n\r\n    Shared with Windows : 6.12 GB\r\n    Backups and Disabled Features : 3.21 GB\r\n    Cache and Temporary Data : 150.00 MB\r\n\r\nDate of Last Cleanup : 2026-09-01 10:00:00\r\n\r\nNumber of Reclaimable Packages : 2\r\nComponent Store Cleanup Recommended : Yes\r\n";
        let c = parse_dism(out);
        assert_eq!(c.reclaimable_packages, 2);
        assert!(c.recommended);
        assert_eq!(c.cache, 150 * 1024 * 1024);
        assert_eq!(c.backups, (3.21 * 1024.0 * 1024.0 * 1024.0) as u64);
        assert_eq!(c.estimate(), c.cache + c.backups);
        assert_eq!(parse_dism_size("0 bytes"), Some(0));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn parses_both_time_formats() {
        assert_eq!(parse_time("1970-01-02T00:00:00Z"), Some(86400));
        assert_eq!(parse_time("2026-10-15 09:20:58 +0000"), parse_time("2026-10-15T09:20:58Z"));
        assert_eq!(parse_time("(null)"), None);
    }
}
