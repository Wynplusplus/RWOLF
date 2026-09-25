//! Loading and decoding of the original Wolfenstein 3D (WL6) data files.
//!
//! No game data is bundled with this project. The user must supply their own
//! legally obtained `WOLF3D` files; see the crate README.
//!
//! SPDX-License-Identifier: MIT

pub mod audio;
pub mod generated;
pub mod map;
pub mod palette;
pub mod vga;
pub mod vswap;

use std::io;
use std::path::{Path, PathBuf};

pub use audio::AudioData;
pub use map::{Map, MapSet};
pub use vga::VgaData;
pub use vswap::{Sprite, VSwap};

/// The file extension used by the registered (WL6) release.
pub const EXT: &str = "WL6";

/// All decoded game data.
pub struct GameData {
    pub dir: PathBuf,
    pub vswap: VSwap,
    pub maps: MapSet,
    pub vga: VgaData,
    pub audio: AudioData,
}

impl GameData {
    pub fn load(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let f = |name: &str| dir.join(format!("{name}.{EXT}"));
        Ok(Self {
            vswap: VSwap::load(f("VSWAP"))?,
            maps: MapSet::load(f("MAPHEAD"), f("GAMEMAPS"))?,
            vga: VgaData::load(f("VGAHEAD"), f("VGAGRAPH"), f("VGADICT"))?,
            audio: AudioData::load(f("AUDIOHED"), f("AUDIOT"))?,
            dir,
        })
    }
}

/// Locate the game data directory.
///
/// Search order:
/// 1. the `WOLF3D_DATA_DIR` environment variable,
/// 2. the `data_dir` from the [config file](crate::config),
/// 3. `./data`,
/// 4. `./WOLF3D`,
/// 5. `~/Downloads/WOLF3D`.
///
/// A `WOLF3D_DATA_DIR` or `data_dir` that points somewhere without the WL6
/// files is reported as a warning rather than silently ignored.
pub fn find_data_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WOLF3D_DATA_DIR") {
        let p = PathBuf::from(dir);
        if is_data_dir(&p) {
            return Some(p);
        }
        eprintln!(
            "warning: WOLF3D_DATA_DIR={} does not contain VSWAP.{EXT}",
            p.display()
        );
    }
    if let Some(p) = crate::config::runtime_data_dir() {
        if is_data_dir(&p) {
            return Some(p);
        }
        eprintln!(
            "warning: selected data_dir {} does not contain VSWAP.{EXT}",
            p.display()
        );
    }
    if let Some(p) = crate::config::get().data_dir.clone() {
        if is_data_dir(&p) {
            return Some(p);
        }
        eprintln!(
            "warning: configured data_dir {} does not contain VSWAP.{EXT}",
            p.display()
        );
    }
    let mut candidates: Vec<PathBuf> = vec![PathBuf::from("data"), PathBuf::from("WOLF3D")];
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join("Downloads/WOLF3D"));
    }
    #[cfg(target_os = "android")]
    {
        // On Android there is no `HOME`. Look in the app-specific external
        // files directory (writable with `adb push` and no permissions
        // prompt), then in the common shared locations. Push the data with:
        //   adb push WOLF3D /sdcard/Android/data/<package>/files/
        candidates.push(PathBuf::from(format!(
            "/sdcard/Android/data/{ANDROID_PACKAGE}/files/WOLF3D"
        )));
        candidates.push(PathBuf::from(format!(
            "/storage/emulated/0/Android/data/{ANDROID_PACKAGE}/files/WOLF3D"
        )));
        candidates.push(PathBuf::from("/sdcard/WOLF3D"));
        candidates.push(PathBuf::from("/storage/emulated/0/WOLF3D"));
        candidates.push(PathBuf::from("/sdcard/Download/WOLF3D"));
    }
    candidates.into_iter().find(|p| is_data_dir(p))
}

/// True when `dir` looks like a Wolfenstein 3D data directory.
pub fn is_data_dir(dir: impl AsRef<Path>) -> bool {
    dir.as_ref().join(format!("VSWAP.{EXT}")).is_file()
}

/// The Android package name, used to locate the app-specific external files
/// directory. Must match `[package.metadata.android] package` in `Cargo.toml`.
pub const ANDROID_PACKAGE: &str = "io.github.wynplusplus.wolf3dbevy";

/// Create the app-specific external files directory on Android so that the
/// user can `adb push` their WL6 data into it without any storage permission.
/// A no-op elsewhere.
pub fn prepare_storage() {
    #[cfg(target_os = "android")]
    {
        let dir = format!("/sdcard/Android/data/{ANDROID_PACKAGE}/files");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("warning: could not create {dir}: {e}");
        }
    }
}
