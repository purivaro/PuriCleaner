//! Removing things, and the shell-script alternative for people who would
//! rather read and run the commands themselves.

use std::fs;
use std::io;
use std::path::Path;

#[cfg(target_os = "macos")]
use trash::macos::{DeleteMethod, TrashContextExtMacos};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Trash,
    Delete,
}

pub fn remove(path: &Path, mode: Mode) -> Result<(), String> {
    let md = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    match mode {
        Mode::Trash => {
            #[allow(unused_mut)]
            let mut ctx = trash::TrashContext::default();
            #[cfg(target_os = "macos")]
            ctx.set_delete_method(DeleteMethod::NsFileManager);
            ctx.delete(path).map_err(|e| e.to_string())
        }
        Mode::Delete => {
            // A symlink is removed as a link; its target is never touched.
            let r: io::Result<()> = if md.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
            r.map_err(|e| e.to_string())
        }
    }
}

/// Move to the Trash through Finder, which asks for the admin password when
/// the item belongs to the system (most apps installed from the App Store).
pub fn trash_with_finder(path: &Path) -> Result<(), String> {
    #[allow(unused_mut)]
    let mut ctx = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    ctx.set_delete_method(DeleteMethod::Finder);
    ctx.delete(path).map_err(|e| e.to_string())
}

pub fn fmt_size(b: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}

pub struct ScriptGroup<'a> {
    pub title: &'a str,
    pub command: Option<&'a str>,
    pub size: u64,
    /// (shell command, bytes) per item.
    pub items: Vec<(String, u64)>,
}

pub fn script(groups: &[ScriptGroup], thai: bool) -> String {
    let total: u64 = groups.iter().map(|g| g.size).sum();
    let count: usize = groups.iter().map(|g| g.items.len()).sum();
    let mut s = String::new();
    if cfg!(windows) {
        s.push_str("# PowerShell\n");
    } else {
        s.push_str("#!/bin/bash\n");
    }
    if thai {
        s.push_str("# Puri Cleaner — สคริปต์ล้างพื้นที่\n");
        s.push_str(&format!("# รวม {} จาก {} รายการ\n", fmt_size(total), count));
        s.push_str("# อ่านทุกบรรทัดก่อนรัน: คำสั่งเหล่านี้ลบถาวร ไม่ผ่านถังขยะ\n");
    } else {
        s.push_str("# Puri Cleaner — cleanup script\n");
        s.push_str(&format!("# {} across {} items\n", fmt_size(total), count));
        s.push_str("# Read every line before running: these commands delete permanently, skipping the Trash.\n");
    }
    if !cfg!(windows) {
        s.push_str("set -u\n");
    }
    for g in groups {
        s.push_str(&format!("\n# ── {} ({}) ", g.title, fmt_size(g.size)));
        s.push_str(&"─".repeat(40usize.saturating_sub(g.title.chars().count())));
        s.push('\n');
        if let Some(cmd) = g.command {
            s.push_str(&format!("# {} {}\n", if thai { "ทางเลือก:" } else { "Alternative:" }, cmd));
        }
        for (line, size) in &g.items {
            // A line may already end in a note ("# Run as administrator").
            let sep = if line.contains("   # ") { ", " } else { "   # " };
            s.push_str(&format!("{line}{sep}{}\n", fmt_size(*size)));
        }
    }
    s.push_str(match (cfg!(windows), thai) {
        (true, true) => "\nWrite-Host \"เสร็จแล้ว\"\n",
        (true, false) => "\nWrite-Host \"Done.\"\n",
        (false, true) => "\necho \"เสร็จแล้ว\"\n",
        (false, false) => "\necho \"Done.\"\n",
    });
    s
}
