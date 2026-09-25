//! Reader for `VSWAP.WL6`, the game's wall-texture and sprite store.
//!
//! Layout (all little-endian):
//!
//! ```text
//! u16  chunk_count
//! u16  sprite_start      // first sprite chunk index
//! u16  sound_start       // first digitised-sound chunk index
//! u32  offsets[chunk_count]
//! u16  lengths[chunk_count]
//! ...  chunk data ...
//! ```
//!
//! * chunks `0 .. sprite_start` are 64x64 wall textures (4096 raw palette
//!   indices, row-major);
//! * chunks `sprite_start .. sound_start` are 64x64 sprites (see
//!   [`Sprite`]);
//! * chunks from `sound_start` on are raw 8-bit unsigned PCM samples.
//!
//! SPDX-License-Identifier: MIT

use std::io;
use std::path::Path;

pub const WALL_SIZE: usize = 64;
pub const WALL_PIXELS: usize = WALL_SIZE * WALL_SIZE;

/// A decoded 64x64 sprite. Pixel index 0 is a valid colour, so a separate
/// opacity mask records which pixels are actually present.
#[derive(Clone)]
pub struct Sprite {
    /// First column (0..=63) that contains any pixels.
    pub left: u8,
    /// Last column (0..=63) that contains any pixels.
    pub right: u8,
    /// Row-major palette indices, `pixels[y * 64 + x]`.
    pub pixels: Box<[u8; WALL_PIXELS]>,
    /// `true` where `pixels` holds a visible pixel.
    pub opaque: Box<[bool; WALL_PIXELS]>,
}

impl Sprite {
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> Option<u8> {
        let i = y * WALL_SIZE + x;
        if self.opaque[i] { Some(self.pixels[i]) } else { None }
    }
}

/// The parsed `VSWAP` file. The raw bytes are retained so chunk access is a
/// cheap slice; decoded sprites are cached separately.
pub struct VSwap {
    data: Vec<u8>,
    offsets: Vec<u32>,
    lengths: Vec<u16>,
    pub sprite_start: u16,
    pub sound_start: u16,
    sprites: Vec<Option<Sprite>>,
}

impl VSwap {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data)
    }

    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        if data.len() < 6 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "VSWAP too small"));
        }
        let chunk_count = u16::from_le_bytes([data[0], data[1]]) as usize;
        let sprite_start = u16::from_le_bytes([data[2], data[3]]);
        let sound_start = u16::from_le_bytes([data[4], data[5]]);

        let off_start = 6;
        let len_start = off_start + 4 * chunk_count;
        let data_start = len_start + 2 * chunk_count;
        if data.len() < data_start {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "VSWAP header truncated",
            ));
        }

        let mut offsets = Vec::with_capacity(chunk_count);
        for i in 0..chunk_count {
            let p = off_start + 4 * i;
            offsets.push(u32::from_le_bytes([
                data[p],
                data[p + 1],
                data[p + 2],
                data[p + 3],
            ]));
        }
        let mut lengths = Vec::with_capacity(chunk_count);
        for i in 0..chunk_count {
            let p = len_start + 2 * i;
            lengths.push(u16::from_le_bytes([data[p], data[p + 1]]));
        }

        let mut sprites = vec![None; chunk_count];
        for i in sprite_start as usize..sound_start as usize {
            if i < chunk_count {
                sprites[i] = decode_sprite(Self::raw_chunk(&data, &offsets, &lengths, i));
            }
        }

        Ok(Self {
            data,
            offsets,
            lengths,
            sprite_start,
            sound_start,
            sprites,
        })
    }

    pub fn chunk_count(&self) -> usize {
        self.offsets.len()
    }

    fn raw_chunk<'a>(
        data: &'a [u8],
        offsets: &[u32],
        lengths: &[u16],
        index: usize,
    ) -> &'a [u8] {
        let off = offsets[index] as usize;
        let len = lengths[index] as usize;
        if off >= data.len() || len == 0 || off + len > data.len() {
            &[]
        } else {
            &data[off..off + len]
        }
    }

    /// Raw bytes of chunk `index`, or an empty slice when the chunk is sparse.
    pub fn chunk(&self, index: usize) -> &[u8] {
        if index >= self.offsets.len() {
            return &[];
        }
        Self::raw_chunk(&self.data, &self.offsets, &self.lengths, index)
    }

    /// A wall texture (4096 bytes, row-major) by *wall number*.
    pub fn wall(&self, wall: usize) -> Option<&[u8]> {
        if wall >= self.sprite_start as usize {
            return None;
        }
        let c = self.chunk(wall);
        (c.len() == WALL_PIXELS).then_some(c)
    }

    /// A decoded sprite by *sprite number* (0-based from `sprite_start`).
    pub fn sprite(&self, sprite: usize) -> Option<&Sprite> {
        let i = self.sprite_start as usize + sprite;
        self.sprites.get(i).and_then(|s| s.as_ref())
    }

    pub fn sprite_count(&self) -> usize {
        self.sound_start as usize - self.sprite_start as usize
    }

    /// Raw digitised-sound chunk (8-bit unsigned PCM at 7000 Hz) by index from
    /// `sound_start`.
    pub fn sound(&self, sound: usize) -> &[u8] {
        self.chunk(self.sound_start as usize + sound)
    }
}

/// Decode one VSWAP sprite.
///
/// The on-disk form is a set of per-column "posts". The header is
/// `left`, `right`, then `right - left + 1` u16 column offsets relative to the
/// chunk. Each column's post list is a run of 3-word entries
/// `(end*2, pixel_base, start*2)` terminated by `end == 0`. A post paints
/// source rows `start .. end` using `pixel_base + row` as the byte offset.
fn decode_sprite(chunk: &[u8]) -> Option<Sprite> {
    let mut pixels = Box::new([0u8; WALL_PIXELS]);
    let mut opaque = Box::new([false; WALL_PIXELS]);
    if chunk.len() < 4 {
        return None;
    }
    let left = u16::from_le_bytes([chunk[0], chunk[1]]) as usize;
    let right = u16::from_le_bytes([chunk[2], chunk[3]]) as usize;
    if right < left || right >= 64 {
        // Sparse/empty sprite.
        return Some(Sprite {
            left: 0,
            right: 0,
            pixels,
            opaque,
        });
    }
    let cols = right - left + 1;
    if chunk.len() < 4 + cols * 2 {
        return None;
    }

    for i in 0..cols {
        let col = left + i;
        let p = 4 + i * 2;
        let mut post = u16::from_le_bytes([chunk[p], chunk[p + 1]]) as usize;
        loop {
            if post + 2 > chunk.len() {
                break;
            }
            let end2 = u16::from_le_bytes([chunk[post], chunk[post + 1]]) as usize;
            if end2 == 0 {
                break;
            }
            if post + 6 > chunk.len() {
                break;
            }
            let pixbase = u16::from_le_bytes([chunk[post + 2], chunk[post + 3]]) as usize;
            let start2 = u16::from_le_bytes([chunk[post + 4], chunk[post + 5]]) as usize;
            post += 6;

            let start = start2 / 2;
            let end = end2 / 2;
            for row in start..end.min(64) {
                let src = pixbase + row;
                if src < chunk.len() {
                    let idx = row * WALL_SIZE + col;
                    pixels[idx] = chunk[src];
                    opaque[idx] = true;
                }
            }
        }
    }

    Some(Sprite {
        left: left as u8,
        right: right as u8,
        pixels,
        opaque,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_when_data_present() {
        let dir = match crate::data::find_data_dir() {
            Some(d) => d,
            None => return,
        };
        let vs = VSwap::load(dir.join("VSWAP.WL6")).expect("load VSWAP");
        assert!(vs.chunk_count() > 100);
        // Wall 0 is a full 64x64 texture.
        let w = vs.wall(0).expect("wall 0");
        assert_eq!(w.len(), WALL_PIXELS);
        // At least one sprite should decode with visible pixels.
        let any = (0..vs.sprite_count()).any(|i| {
            vs.sprite(i)
                .map(|s| s.opaque.iter().any(|&o| o))
                .unwrap_or(false)
        });
        assert!(any, "no sprite decoded with pixels");
    }
}
