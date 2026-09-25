//! Reader for `MAPHEAD.WL6` / `GAMEMAPS.WL6`.
//!
//! `MAPHEAD` starts with the 16-bit RLEW tag (`0xABCD`) followed by 100
//! little-endian 32-bit offsets into `GAMEMAPS` (0 or `0xFFFFFFFF` marks an
//! absent map). Each map header is:
//!
//! ```text
//! u32 planestart[3]
//! u16 planelength[3]
//! u16 width
//! u16 height
//! u8  name[16]
//! ```
//!
//! Each plane is a 16-bit expanded length followed by Carmack-compressed data;
//! the decompressed buffer starts with a length word then RLEW-compressed
//! words. Wolfenstein 3D only uses planes 0 (walls) and 1 (objects).
//!
//! SPDX-License-Identifier: MIT

use std::io;
use std::path::Path;

pub const NUM_MAPS: usize = 100;

/// A single decompressed level.
#[derive(Clone)]
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub name: String,
    /// Plane 0: wall/floor tiles.
    pub walls: Vec<u16>,
    /// Plane 1: objects, actors and markers.
    pub objects: Vec<u16>,
}

impl Map {
    #[inline]
    pub fn wall_at(&self, x: usize, y: usize) -> u16 {
        self.walls[y * self.width + x]
    }
    #[inline]
    pub fn object_at(&self, x: usize, y: usize) -> u16 {
        self.objects[y * self.width + x]
    }
}

pub struct MapSet {
    pub rlew_tag: u16,
    pub maps: Vec<Option<Map>>,
}

impl MapSet {
    pub fn load(head: impl AsRef<Path>, maps: impl AsRef<Path>) -> io::Result<Self> {
        let head = std::fs::read(head)?;
        let maps = std::fs::read(maps)?;
        Self::from_bytes(&head, &maps)
    }

    pub fn from_bytes(head: &[u8], maps: &[u8]) -> io::Result<Self> {
        if head.len() < 2 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "MAPHEAD too small"));
        }
        let rlew_tag = u16::from_le_bytes([head[0], head[1]]);
        let count = ((head.len() - 2) / 4).min(NUM_MAPS);
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let p = 2 + i * 4;
            let off = u32::from_le_bytes([head[p], head[p + 1], head[p + 2], head[p + 3]]);
            if off == 0 || off == 0xFFFF_FFFF || (off as usize) >= maps.len() {
                out.push(None);
                continue;
            }
            out.push(decode_map(maps, off as usize, rlew_tag));
        }
        Ok(Self {
            rlew_tag,
            maps: out,
        })
    }

    pub fn get(&self, index: usize) -> Option<&Map> {
        self.maps.get(index).and_then(|m| m.as_ref())
    }
}

fn u16_at(b: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([b[p], b[p + 1]])
}
fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

fn decode_map(maps: &[u8], off: usize, rlew_tag: u16) -> Option<Map> {
    if off + 38 > maps.len() {
        return None;
    }
    let mut planestart = [0u32; 3];
    for (i, slot) in planestart.iter_mut().enumerate() {
        *slot = u32_at(maps, off + i * 4);
    }
    let mut planelen = [0u16; 3];
    for (i, slot) in planelen.iter_mut().enumerate() {
        *slot = u16_at(maps, off + 12 + i * 2);
    }
    let width = u16_at(maps, off + 18) as usize;
    let height = u16_at(maps, off + 20) as usize;
    let name_bytes = &maps[off + 22..off + 38];
    let name = name_bytes
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as char)
        .collect::<String>();

    if width == 0 || height == 0 {
        return None;
    }
    let count = width * height;

    let plane = |i: usize| -> Vec<u16> {
        let start = planestart[i] as usize;
        let len = planelen[i] as usize;
        if len == 0 || start >= maps.len() || start + len > maps.len() {
            return vec![0; count];
        }
        decode_plane(&maps[start..start + len], rlew_tag, count)
    };

    Some(Map {
        width,
        height,
        name,
        walls: plane(0),
        objects: plane(1),
    })
}

fn decode_plane(raw: &[u8], rlew_tag: u16, count: usize) -> Vec<u16> {
    if raw.len() < 2 {
        return vec![0; count];
    }
    let expanded = u16_at(raw, 0) as usize;
    let words = carmack_expand(&raw[2..], expanded);
    if words.len() < 2 {
        return vec![0; count];
    }
    rlew_expand(&words[1..], count, rlew_tag)
}

/// Expand id Software's Carmack byte/word compression. `expanded_bytes` is the
/// size of the decompressed data.
fn carmack_expand(src: &[u8], expanded_bytes: usize) -> Vec<u16> {
    const NEAR_TAG: u16 = 0xA7;
    const FAR_TAG: u16 = 0xA8;
    let total_words = expanded_bytes / 2;
    let mut out: Vec<u16> = Vec::with_capacity(total_words);
    let mut i = 0usize;
    let get = |i: usize| src.get(i).copied().unwrap_or(0);

    while out.len() < total_words {
        let ch = u16::from_le_bytes([get(i), get(i + 1)]);
        i += 2;
        let high = ch >> 8;
        if high == NEAR_TAG || high == FAR_TAG {
            let count = (ch & 0xFF) as usize;
            if count == 0 {
                // A literal word whose high byte happens to be the tag byte.
                let lo = get(i);
                i += 1;
                out.push(ch | lo as u16);
            } else if high == NEAR_TAG {
                let offset = get(i) as usize;
                i += 1;
                let mut cp = out.len().saturating_sub(offset);
                for _ in 0..count {
                    let v = out.get(cp).copied().unwrap_or(0);
                    out.push(v);
                    cp += 1;
                }
            } else {
                let offset = u16::from_le_bytes([get(i), get(i + 1)]) as usize;
                i += 2;
                let mut cp = offset;
                for _ in 0..count {
                    let v = out.get(cp).copied().unwrap_or(0);
                    out.push(v);
                    cp += 1;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out.truncate(total_words);
    out
}

fn rlew_expand(src: &[u16], count: usize, tag: u16) -> Vec<u16> {
    let mut out = Vec::with_capacity(count);
    let mut i = 0usize;
    while out.len() < count {
        let v = match src.get(i) {
            Some(&v) => v,
            None => break,
        };
        i += 1;
        if v == tag {
            let c = src.get(i).copied().unwrap_or(0) as usize;
            i += 1;
            let val = src.get(i).copied().unwrap_or(0);
            i += 1;
            for _ in 0..c {
                if out.len() >= count {
                    break;
                }
                out.push(val);
            }
        } else {
            out.push(v);
        }
    }
    out.resize(count, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_maps_when_present() {
        let dir = match crate::data::find_data_dir() {
            Some(d) => d,
            None => return,
        };
        let set = MapSet::load(dir.join("MAPHEAD.WL6"), dir.join("GAMEMAPS.WL6")).unwrap();
        assert_eq!(set.rlew_tag, 0xABCD);
        let m = set.get(0).expect("map 0");
        assert_eq!((m.width, m.height), (64, 64));
        assert_eq!(m.walls.len(), 64 * 64);
        // E1M1 has walls and a player start object in the 19..=22 range.
        assert!(m.walls.iter().any(|&t| t > 0 && t < 107));
        assert!(m.objects.iter().any(|&t| (19..=22).contains(&t)));
    }
}
