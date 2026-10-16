//! Facts the file tree alone cannot tell: installed simulator runtimes and
//! when each app was last opened. Gathered once after a scan.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process::Command;

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
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse "2026-10-13T18:24:40Z" or "2026-10-15 09:20:58 +0000" (UTC) to unix seconds.
fn parse_time(s: &str) -> Option<i64> {
    let s = s.trim();
    let num = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    Some(days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se)
}

fn run_json(args: &[&str]) -> Option<Value> {
    let out = Command::new("/usr/bin/xcrun").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

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

/// When the app's program last ran: the newer of Spotlight's "last used"
/// and the executable's access time (Spotlight misses apps not opened via Finder).
fn app_last_used(apps: &[PathBuf]) -> HashMap<PathBuf, i64> {
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

pub fn gather(whole_mac: bool) -> SysInfo {
    if !whole_mac {
        return SysInfo::default();
    }
    let apps = app_bundles();
    SysInfo { runtimes: runtimes(), app_last_used: app_last_used(&apps) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_time_formats() {
        assert_eq!(parse_time("1970-01-02T00:00:00Z"), Some(86400));
        assert_eq!(parse_time("2026-10-15 09:20:58 +0000"), parse_time("2026-10-15T09:20:58Z"));
        assert_eq!(parse_time("(null)"), None);
    }
}
