//! User configuration: where to find the game data.
//!
//! The engine ships no game content. Point it at your own legally obtained
//! Wolfenstein 3D (WL6) files with a small config file:
//!
//! ```toml
//! # wolf3d-bevy.toml
//! data_dir = "/path/to/WOLF3D"
//! ```
//!
//! The file is located (first match wins):
//! 1. `$WOLF3D_CONFIG`
//! 2. `./wolf3d-bevy.toml`
//! 3. `wolf3d-bevy.toml` next to the executable
//! 4. `$XDG_CONFIG_HOME/wolf3d-bevy/config.toml` (or `~/.config/...`)
//!
//! Environment variables still override the config file.
//!
//! SPDX-License-Identifier: MIT

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub const FILE_NAME: &str = "wolf3d-bevy.toml";

#[derive(Debug, Default, Clone)]
pub struct Config {
    /// Path of the file this was loaded from, if any.
    pub source: Option<PathBuf>,
    /// Directory holding the WL6 files (`VSWAP.WL6`, …).
    pub data_dir: Option<PathBuf>,
    /// Default starting episode (1-based).
    pub episode: Option<usize>,
    /// Default starting floor/map (1-based).
    pub map: Option<usize>,
    /// Default difficulty: `baby`, `easy`, `normal` or `hard`.
    pub difficulty: Option<String>,
}

impl Config {
    /// Parse config text. Unknown keys and malformed lines are reported to
    /// stderr and otherwise ignored.
    pub fn parse(text: &str) -> Config {
        let mut cfg = Config::default();
        for (i, raw) in text.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                eprintln!("warning: ignoring malformed config line {}", i + 1);
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = unquote(value.trim());
            match key.as_str() {
                "data_dir" | "data-dir" | "game_dir" | "game-dir" => {
                    cfg.data_dir = Some(expand_tilde(value));
                }
                "episode" => cfg.episode = value.parse().ok(),
                "map" | "floor" => cfg.map = value.parse().ok(),
                "difficulty" => cfg.difficulty = Some(value.to_ascii_lowercase()),
                _ => eprintln!("warning: unknown config key `{key}` on line {}", i + 1),
            }
        }
        cfg
    }
}

static INSTANCE: OnceLock<Config> = OnceLock::new();

/// A data directory chosen in the in-app folder picker during this session.
/// It takes precedence over the config file so the user sees the effect
/// immediately.
static RUNTIME_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// The data directory selected at runtime, if any.
pub fn runtime_data_dir() -> Option<PathBuf> {
    RUNTIME_DATA_DIR
        .get()
        .and_then(|c| c.lock().ok().and_then(|g| g.clone()))
}

/// Remember (and persist) a data directory chosen by the user.
pub fn set_data_dir(dir: PathBuf) {
    let cell = RUNTIME_DATA_DIR.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = cell.lock() {
        *g = Some(dir.clone());
    }
    if let Some(path) = writable_config_path() {
        let text = format!(
            "# Written by ARWOLF when a game folder was selected.\ndata_dir = \"{}\"\n",
            dir.display()
        );
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("warning: could not write config {}: {e}", path.display());
        }
    }
}

/// Where to persist a user-selected directory. On Android that is the
/// app-specific external files folder (always writable); elsewhere the
/// current directory.
fn writable_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        Some(PathBuf::from(format!(
            "/sdcard/Android/data/{}/files/{}",
            crate::data::ANDROID_PACKAGE,
            FILE_NAME
        )))
    }
    #[cfg(not(target_os = "android"))]
    {
        Some(PathBuf::from(FILE_NAME))
    }
}

/// Directory for auxiliary persisted files (high scores, save games).
pub fn persist_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        PathBuf::from(format!(
            "/sdcard/Android/data/{}/files",
            crate::data::ANDROID_PACKAGE
        ))
    }
    #[cfg(not(target_os = "android"))]
    {
        PathBuf::from(".")
    }
}

/// Full path of an auxiliary persisted file, e.g. `wolf3d-bevy.scores`.
pub fn persist_path(name: &str) -> PathBuf {
    persist_dir().join(name)
}

/// The process-wide config, loaded and cached on first use.
pub fn get() -> &'static Config {
    INSTANCE.get_or_init(load)
}

fn load() -> Config {
    for path in candidate_paths() {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut cfg = Config::parse(&text);
                cfg.source = Some(path);
                return cfg;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                eprintln!("warning: could not read config {}: {e}", path.display());
            }
        }
    }
    Config::default()
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(p) = std::env::var("WOLF3D_CONFIG") {
        paths.push(PathBuf::from(p));
    }
    paths.push(PathBuf::from(FILE_NAME));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join(FILE_NAME));
        }
    }
    if let Some(dir) = config_home() {
        paths.push(dir.join("wolf3d-bevy").join("config.toml"));
    }
    #[cfg(target_os = "android")]
    {
        // The file the folder picker writes on Android.
        paths.push(PathBuf::from(format!(
            "/sdcard/Android/data/{}/files/{}",
            crate::data::ANDROID_PACKAGE,
            FILE_NAME
        )));
    }
    paths
}

fn config_home() -> Option<PathBuf> {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(x) if !x.is_empty() => Some(PathBuf::from(x)),
        _ => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")),
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Trim a single pair of matching surrounding straight quotes.
fn unquote(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2
        && ((b[0] == b'"' && b[b.len() - 1] == b'"')
            || (b[0] == b'\'' && b[b.len() - 1] == b'\''))
    {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(value: &str) -> PathBuf {
    if value == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_all_keys_and_comments() {
        let cfg = Config::parse(
            "\
# example config
data_dir = \"/games/WOLF3D\"   # trailing comment
episode = 3
map = 7
difficulty = Hard
",
        );
        assert_eq!(cfg.data_dir.as_deref(), Some(Path::new("/games/WOLF3D")));
        assert_eq!(cfg.episode, Some(3));
        assert_eq!(cfg.map, Some(7));
        assert_eq!(cfg.difficulty.as_deref(), Some("hard"));
    }

    #[test]
    fn accepts_aliases_and_tilde() {
        let cfg = Config::parse("game_dir = ~/wolf\nfloor = 2\n");
        let home = std::env::var_os("HOME").map(PathBuf::from);
        match (cfg.data_dir, home) {
            (Some(dir), Some(home)) => assert_eq!(dir, home.join("wolf")),
            (None, _) => {} // no HOME: tilde left as-is is also acceptable
            other => panic!("unexpected {:?}", other),
        }
        assert_eq!(cfg.map, Some(2));
    }

    #[test]
    fn unquoted_path_with_spaces() {
        let cfg = Config::parse("data_dir = /home/me/My Games/WOLF3D\n");
        assert_eq!(
            cfg.data_dir.as_deref(),
            Some(Path::new("/home/me/My Games/WOLF3D"))
        );
    }
}
