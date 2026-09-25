//! A small in-app directory browser used to choose the game data folder.
//!
//! This is deliberately dependency-free (just `std::fs`) so it works on
//! Android without the Storage Access Framework. On Android 11+ the app needs
//! the "All files access" permission to read shared storage; the UI explains
//! that when a directory cannot be read.
//!
//! SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use bevy::prelude::Resource;

use crate::data::EXT;

/// A subdirectory in the current folder.
#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    /// Whether this directory contains `VSWAP.WL6`.
    pub has_game: bool,
}

/// On-screen geometry, in framebuffer pixels. Shared by the input handling and
/// the renderer so the touch targets line up with what is drawn.
pub mod layout {
    pub const LIST_X: i32 = 4;
    pub const LIST_W: i32 = 288;
    pub const LIST_TOP: i32 = 40;
    pub const ROW_H: i32 = 14;
    pub const VISIBLE_ROWS: usize = 7;

    pub const SCROLL_X: i32 = 296;
    pub const SCROLL_W: i32 = 22;
    pub const SCROLL_UP_Y: i32 = 40;
    pub const SCROLL_UP_H: i32 = 46;
    pub const SCROLL_DOWN_Y: i32 = 92;
    pub const SCROLL_DOWN_H: i32 = 46;

    pub const BUTTON_Y: i32 = 166;
    pub const BUTTON_H: i32 = 26;
    pub const UP_X: i32 = 8;
    pub const UP_W: i32 = 76;
    pub const ROOTS_X: i32 = 92;
    pub const ROOTS_W: i32 = 76;
    pub const USE_X: i32 = 176;
    pub const USE_W: i32 = 136;
    /// "Grant access" button, shown only when a folder could not be read.
    pub const GRANT_Y: i32 = 142;

    /// A row rectangle, or `None` when `index` is not currently visible.
    pub fn row_rect(visible: usize) -> (i32, i32, i32, i32) {
        (
            LIST_X,
            LIST_TOP + visible as i32 * ROW_H,
            LIST_W,
            ROW_H,
        )
    }

    pub fn up_rect() -> (i32, i32, i32, i32) {
        (UP_X, BUTTON_Y, UP_W, BUTTON_H)
    }
    pub fn roots_rect() -> (i32, i32, i32, i32) {
        (ROOTS_X, BUTTON_Y, ROOTS_W, BUTTON_H)
    }
    pub fn use_rect() -> (i32, i32, i32, i32) {
        (USE_X, BUTTON_Y, USE_W, BUTTON_H)
    }
    pub fn scroll_up_rect() -> (i32, i32, i32, i32) {
        (SCROLL_X, SCROLL_UP_Y, SCROLL_W, SCROLL_UP_H)
    }
    pub fn scroll_down_rect() -> (i32, i32, i32, i32) {
        (SCROLL_X, SCROLL_DOWN_Y, SCROLL_W, SCROLL_DOWN_H)
    }

    /// Top-right close button (only active when a game is already running).
    pub fn close_rect() -> (i32, i32, i32, i32) {
        (286, 6, 30, 16)
    }

    /// The "grant access" button, shown when a folder could not be read.
    pub fn grant_rect() -> (i32, i32, i32, i32) {
        (60, GRANT_Y, 200, 18)
    }
}

/// Hit-test a framebuffer point against a rectangle.
pub fn inside(rect: (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    x >= rect.0 as f32
        && y >= rect.1 as f32
        && x < (rect.0 + rect.2) as f32
        && y < (rect.1 + rect.3) as f32
}

#[derive(Resource)]
pub struct Browser {
    pub open: bool,
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub scroll: usize,
    pub status: String,
    pub has_game: bool,
    /// True when the last `refresh` could not read the directory (usually a
    /// missing storage permission on Android).
    pub access_error: bool,
    /// True once we have asked for storage access this session.
    pub access_requested: bool,
    /// Set when the user confirms a folder; the app then loads it.
    pub pending: Option<PathBuf>,
    root_index: usize,
}

/// Directories offered as starting points / quick jumps.
pub fn roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "android")]
    {
        // The app-specific folder (where `adb push` puts data) first, then
        // shared storage and SD cards.
        v.push(PathBuf::from(format!(
            "/sdcard/Android/data/{}/files",
            crate::data::ANDROID_PACKAGE
        )));
        v.push(PathBuf::from("/sdcard/Download"));
        v.push(PathBuf::from("/sdcard/Documents"));
        v.push(PathBuf::from("/storage/emulated/0"));
        v.push(PathBuf::from("/sdcard"));
        v.push(PathBuf::from("/storage"));
    }
    #[cfg(not(target_os = "android"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            v.push(home.join("Downloads"));
            v.push(home.clone());
        }
        v.push(PathBuf::from("/"));
    }
    v
}

impl Default for Browser {
    fn default() -> Self {
        let roots = roots();
        let mut root_index = 0;
        let mut cwd = PathBuf::from("/");
        for (i, r) in roots.iter().enumerate() {
            if r.is_dir() {
                root_index = i;
                cwd = r.clone();
                break;
            }
        }
        let mut b = Self {
            open: false,
            cwd,
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            status: String::new(),
            has_game: false,
            access_error: false,
            access_requested: false,
            pending: None,
            root_index,
        };
        b.refresh();
        b
    }
}

impl Browser {
    pub fn open_at(&mut self, dir: PathBuf) {
        self.open = true;
        self.cwd = dir;
        self.selected = 0;
        self.scroll = 0;
        self.refresh();
    }

    /// Open, preferring the currently configured data directory, then the
    /// first readable root.
    pub fn open_default(&mut self) {
        let mut start = crate::config::runtime_data_dir();
        if start.is_none() {
            if let Some(cfg) = crate::config::get().data_dir.clone() {
                start = Some(cfg);
            }
        }
        let dir = start
            .filter(|d| d.is_dir())
            .unwrap_or_else(|| self.cwd.clone());
        self.open_at(dir);
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        self.access_error = false;
        self.has_game = self.cwd.join(format!("VSWAP.{EXT}")).is_file();
        match std::fs::read_dir(&self.cwd) {
            Ok(read) => {
                for e in read.flatten() {
                    let path = e.path();
                    let is_dir = e
                        .file_type()
                        .map(|t| t.is_dir())
                        .unwrap_or_else(|_| path.is_dir());
                    if !is_dir {
                        continue;
                    }
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        continue;
                    }
                    let has_game = path.join(format!("VSWAP.{EXT}")).is_file();
                    self.entries.push(Entry {
                        name,
                        path,
                        has_game,
                    });
                }
                self.entries.sort_by(|a, b| {
                    b.has_game
                        .cmp(&a.has_game)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
                self.status = if self.has_game {
                    format!("FOUND VSWAP.{EXT} HERE - PRESS USE THIS FOLDER")
                } else {
                    "CHOOSE A FOLDER CONTAINING VSWAP.WL6".to_string()
                };
            }
            Err(e) => {
                self.status = format!("CANNOT READ THIS FOLDER: {e}");
                self.access_error = true;
            }
        }
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
        self.clamp_scroll();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    pub fn select_delta(&mut self, delta: i32) {
        if self.entries.is_empty() {
            return;
        }
        let n = self.entries.len() as i32;
        let mut s = self.selected as i32 + delta;
        s = s.clamp(0, n - 1);
        self.selected = s as usize;
        self.ensure_visible();
    }

    pub fn scroll_delta(&mut self, delta: i32) {
        let max = self.entries.len().saturating_sub(layout::VISIBLE_ROWS) as i32;
        let mut s = self.scroll as i32 + delta;
        s = s.clamp(0, max.max(0));
        self.scroll = s as usize;
    }

    fn clamp_scroll(&mut self) {
        let max = self.entries.len().saturating_sub(layout::VISIBLE_ROWS);
        if self.scroll > max {
            self.scroll = max;
        }
    }

    fn ensure_visible(&mut self) {
        let vis = layout::VISIBLE_ROWS;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + vis {
            self.scroll = self.selected + 1 - vis;
        }
        self.clamp_scroll();
    }

    /// Navigate into the selected directory.
    pub fn enter_selected(&mut self) {
        if let Some(e) = self.entries.get(self.selected).cloned() {
            self.cwd = e.path;
            self.selected = 0;
            self.scroll = 0;
            self.refresh();
        }
    }

    /// Go to the parent directory.
    pub fn go_up(&mut self) {
        if let Some(parent) = self.cwd.parent() {
            let parent = parent.to_path_buf();
            self.cwd = parent;
            self.selected = 0;
            self.scroll = 0;
            self.refresh();
        }
    }

    /// Jump to the next quick-jump root.
    pub fn next_root(&mut self) {
        let roots = roots();
        if roots.is_empty() {
            return;
        }
        for _ in 0..roots.len() {
            self.root_index = (self.root_index + 1) % roots.len();
            let r = &roots[self.root_index];
            if r.is_dir() {
                self.cwd = r.clone();
                self.selected = 0;
                self.scroll = 0;
                self.refresh();
                return;
            }
        }
        self.status = "NO READABLE FOLDERS - SEE README FOR PERMISSIONS".to_string();
    }

    /// Confirm the current folder if it contains the game data.
    pub fn confirm(&mut self) -> Option<PathBuf> {
        if self.has_game {
            Some(self.cwd.clone())
        } else {
            self.status = format!("NO VSWAP.{EXT} IN THIS FOLDER");
            None
        }
    }

    /// Whether `dir` looks like a game data directory (used by tests).
    pub fn is_game_dir(dir: &Path) -> bool {
        dir.join(format!("VSWAP.{EXT}")).is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_hit_testing() {
        assert!(inside(layout::up_rect(), 20.0, 175.0));
        assert!(!inside(layout::up_rect(), 300.0, 175.0));
        let r = layout::row_rect(0);
        assert!(inside(r, 10.0, r.1 as f32 + 1.0));
    }

    #[test]
    fn browser_starts_and_refreshes() {
        let mut b = Browser::default();
        // Should not panic even if the starting directory is unreadable.
        b.refresh();
        b.select_delta(1);
        b.scroll_delta(1);
        b.go_up();
        b.next_root();
    }
}
