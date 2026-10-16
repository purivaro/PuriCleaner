//! In-memory picture of the scanned folder: a flat arena of nodes.
//!
//! Every directory becomes a node. Files only become nodes when they are
//! large enough to matter on a treemap (`BIG_FILE`); smaller files are summed
//! into their directory's `own` size. Parents always come before their
//! children in `nodes`, so a reverse pass is a bottom-up pass.

use std::path::PathBuf;

pub const NONE: u32 = u32::MAX;
pub const BIG_FILE: u64 = 4 * 1024 * 1024;
pub const NCAT: usize = 8;

pub mod flag {
    pub const FILE: u16 = 1 << 0;
    pub const GIT: u16 = 1 << 1;
    pub const PKG_JSON: u16 = 1 << 2;
    pub const CARGO: u16 = 1 << 3;
    pub const PY_PROJECT: u16 = 1 << 4;
    pub const GO_MOD: u16 = 1 << 5;
    pub const PYVENV: u16 = 1 << 6;
    pub const UNREADABLE: u16 = 1 << 7;
    pub const REMOVED: u16 = 1 << 8;
    pub const OTHER_PROJECT: u16 = 1 << 9;
    pub const PUBSPEC: u16 = 1 << 10;
    pub const GRADLE: u16 = 1 << 11;
    pub const PODFILE: u16 = 1 << 12;
    pub const SWIFTPM: u16 = 1 << 13;
    pub const PROJECT: u16 = GIT | PKG_JSON | CARGO | PY_PROJECT | GO_MOD | OTHER_PROJECT;
}

/// Marker files that tell us what kind of folder we are looking at.
pub fn marker_flag(name: &str) -> u16 {
    match name {
        ".git" => flag::GIT,
        "package.json" => flag::PKG_JSON,
        "Cargo.toml" => flag::CARGO,
        "pyproject.toml" | "requirements.txt" | "setup.py" | "Pipfile" => flag::PY_PROJECT,
        "go.mod" => flag::GO_MOD,
        "pyvenv.cfg" => flag::PYVENV,
        "pubspec.yaml" => flag::OTHER_PROJECT | flag::PUBSPEC,
        "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => {
            flag::OTHER_PROJECT | flag::GRADLE
        }
        "Podfile" => flag::OTHER_PROJECT | flag::PODFILE,
        "Package.swift" => flag::OTHER_PROJECT | flag::SWIFTPM,
        "composer.json" | "Gemfile" | "pom.xml" => flag::OTHER_PROJECT,
        _ => 0,
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: Box<str>,
    pub parent: u32,
    pub children: Vec<u32>,
    /// Bytes allocated on disk for the whole subtree.
    pub size: u64,
    /// Bytes of small files sitting directly in this directory.
    pub own: u64,
    pub own_count: u64,
    /// Number of files in the whole subtree.
    pub files: u64,
    /// Newest file modification time (unix seconds) in the subtree.
    pub newest: i64,
    /// Newest modification time among files directly in this directory.
    pub own_newest: i64,
    pub flags: u16,
    pub cat: u8,
    /// Subtree bytes broken down by category.
    pub cats: [u64; NCAT],
}

impl Node {
    pub fn is_file(&self) -> bool {
        self.flags & flag::FILE != 0
    }
    pub fn is_removed(&self) -> bool {
        self.flags & flag::REMOVED != 0
    }
}

pub struct Tree {
    /// Path of node 0. In whole-Mac mode this is `/` and node 0 is a virtual
    /// folder whose children are the scanned roots, named by their path
    /// below `/` (e.g. "Applications", "opt/homebrew", "Users/me").
    pub root: PathBuf,
    pub home: PathBuf,
    /// Node of the home folder (0 when only one folder was scanned).
    pub home_id: u32,
    pub nodes: Vec<Node>,
    /// Folders under home that could not be read (fixable with Full Disk Access).
    pub unreadable: u64,
}

impl Tree {
    pub fn node(&self, id: u32) -> &Node {
        &self.nodes[id as usize]
    }

    /// Path components below the root, outermost first.
    fn components(&self, mut id: u32) -> Vec<&str> {
        let mut parts = Vec::new();
        while id != 0 && id != NONE {
            let n = self.node(id);
            parts.push(&*n.name);
            id = n.parent;
        }
        parts.reverse();
        parts
    }

    pub fn path(&self, id: u32) -> PathBuf {
        let mut p = self.root.clone();
        for c in self.components(id) {
            // Top-level names can hold several parts ("Users/me").
            for part in c.split('/') {
                p.push(part);
            }
        }
        p
    }

    /// Path relative to the root, `/`-joined ("" for the root itself).
    pub fn rel(&self, id: u32) -> String {
        self.components(id).join("/")
    }

    /// Human path, with the home folder shown as `~`.
    pub fn display(&self, id: u32) -> String {
        let p = self.path(id);
        match p.strip_prefix(&self.home) {
            Ok(r) if r.as_os_str().is_empty() => "~".into(),
            Ok(r) => format!("~{}{}", std::path::MAIN_SEPARATOR, r.display()),
            Err(_) => p.display().to_string(),
        }
    }

    pub fn child(&self, id: u32, name: &str) -> Option<u32> {
        self.node(id)
            .children
            .iter()
            .copied()
            .find(|&c| &*self.node(c).name == name && !self.node(c).is_removed())
    }

    /// Find a node by a home-relative path like `Library/Caches`.
    pub fn lookup(&self, rel: &str) -> Option<u32> {
        self.walk(self.home_id, rel)
    }

    fn walk(&self, from: u32, rel: &str) -> Option<u32> {
        let mut id = from;
        for part in rel.split('/').filter(|s| !s.is_empty()) {
            id = self.child(id, part)?;
        }
        Some(id)
    }

    /// Find a node by absolute path, e.g. `/Library/Developer/CoreSimulator`.
    pub fn lookup_abs(&self, path: &std::path::Path) -> Option<u32> {
        let rest = path.strip_prefix(&self.root).ok()?;
        let rest = rest.to_string_lossy().replace('\\', "/");
        // Top-level names may span several components ("opt/homebrew").
        for &c in &self.node(0).children {
            let name = &*self.node(c).name;
            if self.node(c).is_removed() {
                continue;
            }
            if *rest == *name {
                return Some(c);
            }
            if let Some(tail) = rest.strip_prefix(name).and_then(|t| t.strip_prefix('/')) {
                return self.walk(c, tail);
            }
        }
        if rest.is_empty() {
            Some(0)
        } else {
            self.walk(0, &rest)
        }
    }

    pub fn ancestors(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        let mut cur = self.node(id).parent;
        std::iter::from_fn(move || {
            if cur == NONE {
                return None;
            }
            let out = cur;
            cur = self.node(cur).parent;
            Some(out)
        })
    }

    pub fn is_under(&self, id: u32, ancestor: u32) -> bool {
        id == ancestor || self.ancestors(id).any(|a| a == ancestor)
    }

    /// Category that takes the most space inside this node.
    pub fn dominant(&self, id: u32) -> u8 {
        let n = self.node(id);
        let mut best = n.cat as usize;
        for c in 0..NCAT {
            if n.cats[c] > n.cats[best] {
                best = c;
            }
        }
        best as u8
    }

    /// Drop a node from the tree (after it was deleted on disk).
    pub fn remove(&mut self, id: u32) {
        let (size, files, cats, parent) = {
            let n = self.node(id);
            (n.size, n.files, n.cats, n.parent)
        };
        let ups: Vec<u32> = self.ancestors(id).collect();
        for a in ups {
            let n = &mut self.nodes[a as usize];
            n.size = n.size.saturating_sub(size);
            n.files = n.files.saturating_sub(files);
            for c in 0..NCAT {
                n.cats[c] = n.cats[c].saturating_sub(cats[c]);
            }
        }
        if parent != NONE {
            self.nodes[parent as usize].children.retain(|&c| c != id);
        }
        self.nodes[id as usize].flags |= flag::REMOVED;
    }

    /// Replace a node's measured size (after a partial delete or a move into
    /// it), collapsing its contents into a single blob.
    pub fn set_size(&mut self, id: u32, size: u64, files: u64) {
        let (old, old_files, old_cats, cat) = {
            let n = self.node(id);
            (n.size, n.files, n.cats, n.cat as usize)
        };
        let mut new_cats = [0u64; NCAT];
        new_cats[cat] = size;
        let ups: Vec<u32> = self.ancestors(id).collect();
        for a in ups {
            let n = &mut self.nodes[a as usize];
            n.size = n.size.saturating_sub(old) + size;
            n.files = n.files.saturating_sub(old_files) + files;
            for c in 0..NCAT {
                n.cats[c] = n.cats[c].saturating_sub(old_cats[c]) + new_cats[c];
            }
        }
        let kids = std::mem::take(&mut self.nodes[id as usize].children);
        for k in kids {
            self.mark_removed(k);
        }
        let n = &mut self.nodes[id as usize];
        n.size = size;
        n.own = size;
        n.own_count = files;
        n.files = files;
        n.cats = new_cats;
    }

    fn mark_removed(&mut self, id: u32) {
        let mut stack = vec![id];
        while let Some(i) = stack.pop() {
            let n = &mut self.nodes[i as usize];
            n.flags |= flag::REMOVED;
            stack.extend(n.children.iter().copied());
        }
    }
}
