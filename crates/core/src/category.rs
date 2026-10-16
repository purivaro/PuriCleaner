//! Colour groups for the treemap ("Documents & media", "Code projects", ...).
//!
//! Well-known folders get an explicit group; anything containing a project
//! marker (.git, package.json, Cargo.toml, ...) becomes a code project;
//! everything else inherits its parent's group.

use crate::tree::{flag, Tree, NCAT, NONE};

pub const DOCS: u8 = 0;
pub const DEV: u8 = 1;
pub const CODE: u8 = 2;
pub const SDK: u8 = 3;
pub const AI: u8 = 4;
pub const PKG: u8 = 5;
pub const APPS: u8 = 6;
pub const SYS: u8 = 7;

pub const KEYS: [&str; NCAT] = ["docs", "dev", "code", "sdk", "ai", "pkg", "apps", "sys"];

/// Groups for folders outside home, by path below the system drive.
#[cfg(windows)]
const SYSTEM: &[(&str, u8)] = &[
    ("Program Files", APPS),
    ("Program Files (x86)", APPS),
    ("ProgramData", SYS),
    ("ProgramData/Microsoft/Windows/WER", SYS),
];

/// Explicit groups by home-relative path on Windows.
#[cfg(windows)]
const EXPLICIT: &[(&str, u8)] = &[
    (".ollama", AI),
    (".lmstudio", AI),
    (".cache/huggingface", AI),
    ("AppData/Local/Android", SDK),
    (".android", SDK),
    ("AppData/Local/npm-cache", PKG),
    ("AppData/Local/Yarn", PKG),
    ("AppData/Local/pnpm", PKG),
    ("AppData/Local/pnpm-cache", PKG),
    ("AppData/Local/pip", PKG),
    ("AppData/Local/uv", PKG),
    ("AppData/Local/NuGet", PKG),
    (".nuget", PKG),
    (".gradle", PKG),
    (".m2", PKG),
    (".cargo", PKG),
    (".cache", PKG),
    ("go", PKG),
    ("AppData", DEV),
    (".rustup", DEV),
    (".vscode", DEV),
    (".docker", DEV),
    (".claude", DEV),
    ("Documents", DOCS),
    ("Desktop", DOCS),
    ("Pictures", DOCS),
    ("Music", DOCS),
    ("Videos", DOCS),
    ("Downloads", DOCS),
    ("OneDrive", DOCS),
    ("source", CODE),
    ("Dev", CODE),
    ("dev", CODE),
    ("Projects", CODE),
    ("projects", CODE),
    ("Code", CODE),
    ("code", CODE),
    ("src", CODE),
    ("repos", CODE),
    ("GitHub", CODE),
    ("workspace", CODE),
];

/// Groups for folders outside home, by path below `/`.
#[cfg(not(windows))]
const SYSTEM: &[(&str, u8)] = &[
    ("Applications", APPS),
    ("Library", SYS),
    ("Library/Developer", SDK),
    ("Library/Application Support/GarageBand", APPS),
    ("Library/Application Support/Logic", APPS),
    ("Library/Audio/Apple Loops", APPS),
    ("opt/homebrew", DEV),
    ("usr/local", DEV),
    ("private/var/folders", SYS),
];

/// Explicit groups by home-relative path. Checked at depth <= 4 only.
#[cfg(not(windows))]
const EXPLICIT: &[(&str, u8)] = &[
    // AI models
    (".ollama", AI),
    (".lmstudio", AI),
    (".cache/lm-studio", AI),
    (".cache/huggingface", AI),
    (".cache/torch", AI),
    (".cache/whisper", AI),
    ("Library/Application Support/LM Studio", AI),
    ("Library/Application Support/ollama", AI),
    ("Library/Application Support/DiffusionBee", AI),
    // iOS & Android SDKs
    ("Library/Android", SDK),
    (".android", SDK),
    ("Library/Developer", SDK),
    ("Library/Application Support/Google/AndroidStudio", SDK),
    ("flutter", SDK),
    ("fvm", SDK),
    // Package caches
    (".npm", PKG),
    (".gradle", PKG),
    (".cache", PKG),
    (".cargo", PKG),
    (".bun", PKG),
    (".pnpm-store", PKG),
    ("Library/pnpm", PKG),
    (".m2", PKG),
    ("go", PKG),
    (".yarn", PKG),
    (".composer", PKG),
    (".nuget", PKG),
    (".pub-cache", PKG),
    (".cocoapods", PKG),
    ("Library/Caches", PKG),
    (".conda", PKG),
    ("miniconda3/pkgs", PKG),
    ("anaconda3/pkgs", PKG),
    // Dev tools & other
    (".rustup", DEV),
    (".vscode", DEV),
    (".cursor", DEV),
    (".claude", DEV),
    (".docker", DEV),
    ("Library/Containers/com.docker.docker", DEV),
    ("Applications", DEV),
    ("Library", DEV),
    // Documents & media
    ("Documents", DOCS),
    ("Desktop", DOCS),
    ("Pictures", DOCS),
    ("Music", DOCS),
    ("Movies", DOCS),
    ("Downloads", DOCS),
    ("Public", DOCS),
    (".Trash", DOCS),
    ("Library/Mobile Documents", DOCS),
    ("Library/CloudStorage", DOCS),
    ("Library/Mail", DOCS),
    ("Library/Messages", DOCS),
    ("Library/Photos", DOCS),
    ("Library/Application Support/MobileSync", DOCS),
    ("Library/Group Containers/group.com.apple.notes", DOCS),
    // Code projects (common workspace folder names)
    ("Dev", CODE),
    ("dev", CODE),
    ("Developer", CODE),
    ("Projects", CODE),
    ("projects", CODE),
    ("Code", CODE),
    ("code", CODE),
    ("src", CODE),
    ("repos", CODE),
    ("Repos", CODE),
    ("workspace", CODE),
    ("Workspace", CODE),
    ("Sites", CODE),
    ("GitHub", CODE),
    ("git", CODE),
];

fn explicit(rel: &str) -> Option<u8> {
    EXPLICIT.iter().find(|(p, _)| *p == rel).map(|&(_, c)| c)
}

fn system(rel: &str) -> Option<u8> {
    SYSTEM.iter().find(|(p, _)| *p == rel).map(|&(_, c)| c).or_else(|| {
        // The per-user temp folder's name is random: private/var/folders/xx/yyyy.
        rel.starts_with("private/var/folders/").then_some(SYS)
    })
}

pub fn classify(tree: &mut Tree) {
    let n = tree.nodes.len();
    let whole_mac = tree.home_id != 0;
    let home_id = tree.home_id as usize;
    // Path below the root for shallow nodes only; deeper ones never match.
    // Top-level names may already hold several components ("Users/me").
    let mut rel: Vec<Option<String>> = vec![None; n];
    let mut depth = vec![0u8; n];
    rel[0] = Some(String::new());
    for i in 0..n {
        let parent = tree.nodes[i].parent;
        let parent_cat = if parent == NONE { SYS } else { tree.nodes[parent as usize].cat };
        if parent != NONE {
            let p = parent as usize;
            depth[i] = depth[p].saturating_add(1);
            if depth[i] <= 6 {
                if let Some(pr) = &rel[p] {
                    let name = &tree.nodes[i].name;
                    rel[i] = Some(if pr.is_empty() { name.to_string() } else { format!("{pr}/{name}") });
                }
            }
        }
        let node = &tree.nodes[i];
        let mut cat = None;
        if i == home_id {
            cat = Some(DEV);
        } else if let Some(r) = rel[i].as_deref() {
            // Below home: match the home table on the home-relative path.
            let home_rel = rel[home_id].as_deref().unwrap_or("");
            let in_home = if home_rel.is_empty() { Some(r) } else { r.strip_prefix(home_rel).and_then(|t| t.strip_prefix('/')) };
            cat = match in_home {
                Some(h) if tree.root == tree.home || whole_mac => explicit(h),
                Some(_) => None,
                None => system(r),
            };
        }
        if cat.is_none()
            && !node.is_file()
            && node.flags & flag::PROJECT != 0
            && matches!(parent_cat, DOCS | DEV)
            && tree.is_under(i as u32, tree.home_id)
        {
            cat = Some(CODE);
        }
        let fallback = if i == 0 { if whole_mac { SYS } else { DEV } } else { parent_cat };
        tree.nodes[i].cat = cat.unwrap_or(fallback);
    }
    // Bottom-up size per category.
    for i in (0..n).rev() {
        let node = &mut tree.nodes[i];
        let c = node.cat as usize;
        node.cats[c] += if node.is_file() { node.size } else { node.own };
        let cats = node.cats;
        let parent = node.parent;
        if parent != NONE {
            let p = &mut tree.nodes[parent as usize];
            for k in 0..NCAT {
                p.cats[k] += cats[k];
            }
        }
    }
}
