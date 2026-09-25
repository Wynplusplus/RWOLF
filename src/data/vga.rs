//! Reader for `VGAHEAD.WL6`, `VGAGRAPH.WL6` and `VGADICT.WL6` — the indexed
//! graphics used for the HUD, fonts, pictures and 8x8 tiles.
//!
//! `VGAHEAD` holds `NUMCHUNKS + 1` 24-bit little-endian file offsets (the value
//! `0xFFFFFF` marks a sparse chunk). `VGAGRAPH` holds Huffman-compressed
//! chunks; the dictionary lives in `VGADICT` as 256 nodes of two 16-bit
//! children. A child below 256 is a literal byte, otherwise it is the index of
//! the next node minus 256. Node 254 is the root. Bits are read most
//! significant first.
//!
//! SPDX-License-Identifier: MIT

use std::io;
use std::path::Path;

pub const NUM_CHUNKS: usize = 149;
pub const NUM_PICS: usize = 132;
pub const STRUCT_PIC: usize = 0;
pub const START_FONT: usize = 1;
pub const START_PICS: usize = 3;
pub const START_TILE8: usize = 135;
pub const START_EXTERNS: usize = 136;
pub const NUM_TILE8: usize = 72;
pub const TILE8_BYTES: usize = 64;

/// Named picture numbers used by the engine. These are indices into
/// [`VgaData::pic`] (i.e. original chunk number minus `START_PICS`).
pub mod pic {
    pub const STATUS_BAR: usize = 86 - super::START_PICS;
    pub const TITLE: usize = 87 - super::START_PICS;
    pub const KNIFE: usize = 91 - super::START_PICS;
    pub const GUN: usize = 92 - super::START_PICS;
    pub const MACHINE_GUN: usize = 93 - super::START_PICS;
    pub const GATLING_GUN: usize = 94 - super::START_PICS;
    pub const NO_KEY: usize = 95 - super::START_PICS;
    pub const GOLD_KEY: usize = 96 - super::START_PICS;
    pub const SILVER_KEY: usize = 97 - super::START_PICS;
    pub const N_BLANK: usize = 98 - super::START_PICS;
    pub const N_0: usize = 99 - super::START_PICS;
    pub const N_9: usize = 108 - super::START_PICS;
    pub const FACE_1A: usize = 109 - super::START_PICS;
    pub const FACE_8A: usize = 130 - super::START_PICS;
    pub const GOT_GATLING: usize = 131 - super::START_PICS;
    pub const MUTANT_BJ: usize = 132 - super::START_PICS;
    pub const PAUSED: usize = 133 - super::START_PICS;
    pub const GET_PSYCHED: usize = 134 - super::START_PICS;
}

#[derive(Clone)]
pub struct Pic {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

#[derive(Clone)]
pub struct Font {
    pub height: usize,
    pub widths: [u8; 256],
    pub locations: [u16; 256],
    pub data: Vec<u8>,
}

impl Font {
    /// Width in pixels of `ch`, or `None` when the glyph has no width.
    pub fn char_width(&self, ch: u8) -> usize {
        self.widths[ch as usize] as usize
    }

    /// Returns `(width, height, pixel predicate)` for a glyph, or `None`.
    pub fn glyph(&self, ch: u8) -> Option<(usize, usize)> {
        let w = self.char_width(ch);
        if w == 0 {
            return None;
        }
        Some((w, self.height))
    }

    /// Palette-index value (0/1) of glyph pixel `(x, y)`, or `None` if out of
    /// range.
    pub fn glyph_pixel(&self, ch: u8, x: usize, y: usize) -> Option<bool> {
        let w = self.char_width(ch);
        if w == 0 || x >= w || y >= self.height {
            return None;
        }
        let off = self.locations[ch as usize] as usize + y * w + x;
        self.data.get(off).map(|&b| b != 0)
    }
}

pub struct VgaData {
    pics: Vec<Option<Pic>>,
    pub fonts: Vec<Font>,
    pub tile8: Vec<[u8; TILE8_BYTES]>,
}

impl VgaData {
    pub fn load(head: impl AsRef<Path>, graph: impl AsRef<Path>, dict: impl AsRef<Path>) -> io::Result<Self> {
        let head = std::fs::read(head)?;
        let graph = std::fs::read(graph)?;
        let dict = std::fs::read(dict)?;
        Self::from_bytes(&head, &graph, &dict)
    }

    pub fn from_bytes(head: &[u8], graph: &[u8], dict: &[u8]) -> io::Result<Self> {
        if dict.len() < 256 * 4 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "VGADICT too small"));
        }
        let mut nodes = [(0u16, 0u16); 256];
        for (i, node) in nodes.iter_mut().enumerate() {
            let p = i * 4;
            *node = (
                u16::from_le_bytes([dict[p], dict[p + 1]]),
                u16::from_le_bytes([dict[p + 2], dict[p + 3]]),
            );
        }

        // 24-bit offsets for NUMCHUNKS + 1 entries.
        let n_off = head.len() / 3;
        let mut offsets = Vec::with_capacity(n_off);
        for i in 0..n_off {
            let p = i * 3;
            let v = (head[p] as u32) | ((head[p + 1] as u32) << 8) | ((head[p + 2] as u32) << 16);
            offsets.push(if v == 0xFF_FFFF { None } else { Some(v as usize) });
        }

        let chunk_bytes = |chunk: usize| -> Option<Vec<u8>> {
            let pos = (*offsets.get(chunk)?)?;
            let mut next = chunk + 1;
            while next < offsets.len() && offsets[next].is_none() {
                next += 1;
            }
            let end = (*offsets.get(next)?)?;
            if end <= pos || end > graph.len() {
                return None;
            }
            Some(graph[pos..end].to_vec())
        };

        // Pictable: dimensions of every picture.
        let mut dims = vec![(0usize, 0usize); NUM_PICS];
        if let Some(raw) = chunk_bytes(STRUCT_PIC) {
            if let Some(expanded) = expand_chunk(STRUCT_PIC, &raw, &nodes) {
                for (i, d) in dims.iter_mut().enumerate() {
                    let p = i * 4;
                    if p + 4 <= expanded.len() {
                        *d = (
                            u16::from_le_bytes([expanded[p], expanded[p + 1]]) as usize,
                            u16::from_le_bytes([expanded[p + 2], expanded[p + 3]]) as usize,
                        );
                    }
                }
            }
        }

        let mut pics = Vec::with_capacity(NUM_PICS);
        for (i, &(w, h)) in dims.iter().enumerate() {
            let pic = chunk_bytes(START_PICS + i)
                .and_then(|raw| expand_chunk(START_PICS + i, &raw, &nodes))
                .and_then(|data| {
                    if w == 0 || h == 0 || data.len() < w * h {
                        None
                    } else {
                        Some(Pic {
                            width: w,
                            height: h,
                            pixels: deplane(&data, w, h),
                        })
                    }
                });
            pics.push(pic);
        }

        // Fonts.
        let mut fonts = Vec::new();
        for f in 0..2 {
            let chunk = START_FONT + f;
            let font = chunk_bytes(chunk)
                .and_then(|raw| expand_chunk(chunk, &raw, &nodes))
                .and_then(parse_font);
            if let Some(font) = font {
                fonts.push(font);
            }
        }

        // The 8x8 tile bank is one implicit-size chunk: no length prefix, and
        // the expanded size is known (NUM_TILE8 * 64).
        let mut tile8 = Vec::new();
        if let Some(raw) = chunk_bytes(START_TILE8) {
            let data = huff_expand(&raw, 0, &nodes, NUM_TILE8 * TILE8_BYTES);
            for t in 0..NUM_TILE8 {
                let p = t * TILE8_BYTES;
                if p + TILE8_BYTES <= data.len() {
                    let mut tile = [0u8; TILE8_BYTES];
                    tile.copy_from_slice(&data[p..p + TILE8_BYTES]);
                    tile8.push(tile);
                }
            }
        }

        Ok(Self { pics, fonts, tile8 })
    }

    pub fn pic(&self, index: usize) -> Option<&Pic> {
        self.pics.get(index).and_then(|p| p.as_ref())
    }

    pub fn font(&self, index: usize) -> Option<&Font> {
        self.fonts.get(index)
    }
}

fn parse_font(data: Vec<u8>) -> Option<Font> {
    if data.len() < 2 + 512 + 256 {
        return None;
    }
    let height = u16::from_le_bytes([data[0], data[1]]) as usize;
    let mut locations = [0u16; 256];
    for (i, l) in locations.iter_mut().enumerate() {
        let p = 2 + i * 2;
        *l = u16::from_le_bytes([data[p], data[p + 1]]);
    }
    let mut widths = [0u8; 256];
    widths.copy_from_slice(&data[514..770]);
    Some(Font {
        height,
        widths,
        locations,
        data,
    })
}

fn expand_chunk(chunk: usize, raw: &[u8], nodes: &[(u16, u16); 256]) -> Option<Vec<u8>> {
    if chunk >= START_TILE8 && chunk < START_EXTERNS {
        // Implicit length (the tile bank); handled separately by the caller.
        return Some(huff_expand(raw, 0, nodes, NUM_TILE8 * TILE8_BYTES));
    }
    if raw.len() < 4 {
        return None;
    }
    let expanded = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    Some(huff_expand(&raw[4..], 0, nodes, expanded))
}

/// Convert one of the original's 4-plane (unchained VGA) pictures into linear
/// chunky pixels.
///
/// The source is laid out as four planes, one per `x mod 4`, each holding
/// `width/4` bytes per row for every row. See `VL_MemToScreen` in the original.
fn deplane(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    if width % 4 != 0 || data.len() < width * height {
        let n = (width * height).min(data.len());
        let mut v = data[..n].to_vec();
        v.resize(width * height, 0);
        return v;
    }
    let w4 = width / 4;
    let plane = w4 * height;
    let mut out = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            let p = x & 3;
            out[y * width + x] = data[p * plane + y * w4 + (x >> 2)];
        }
    }
    out
}

/// Huffman-decode `src` into exactly `expanded` bytes using `nodes` (root 254,
/// bits least-significant first).
pub fn huff_expand(src: &[u8], _offset: usize, nodes: &[(u16, u16); 256], expanded: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(expanded);
    let mut node = 254usize;
    let mut bit = 0usize;
    while out.len() < expanded {
        let byte = src.get(bit >> 3).copied().unwrap_or(0);
        let set = (byte >> (bit & 7)) & 1 != 0;
        bit += 1;
        let next = if set { nodes[node].1 } else { nodes[node].0 };
        if next < 256 {
            out.push(next as u8);
            node = 254;
        } else {
            node = (next - 256) as usize;
            if node > 254 {
                node = 254;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_graphics_when_present() {
        let dir = match crate::data::find_data_dir() {
            Some(d) => d,
            None => return,
        };
        let vga = VgaData::load(
            dir.join("VGAHEAD.WL6"),
            dir.join("VGAGRAPH.WL6"),
            dir.join("VGADICT.WL6"),
        )
        .unwrap();
        let bar = vga.pic(pic::STATUS_BAR).expect("status bar");
        assert_eq!((bar.width, bar.height), (320, 40));
        assert_eq!(vga.tile8.len(), NUM_TILE8);
        assert!(!vga.fonts.is_empty());
        // The title picture exists too.
        assert!(vga.pic(pic::TITLE).is_some());
    }
}
