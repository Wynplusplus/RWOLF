//! An indexed-colour software framebuffer, matching the original 320x200 VGA
//! screen.
//!
//! SPDX-License-Identifier: MIT

use crate::data::palette;

pub const VIEW_W: usize = 320;
pub const VIEW_H: usize = 200;
/// Height of the 3D viewport (the status bar occupies the rest).
pub const VIEW_3D_H: usize = 160;
pub const STATUS_H: usize = 40;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, color: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.pixels[y as usize * self.width + x as usize] = color;
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    pub fn clear(&mut self, color: u8) {
        self.pixels.fill(color);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u8) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, color);
            }
        }
    }

    /// Draw a filled circle (used by the touch-control overlay).
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, color: u8) {
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r2 {
                    self.put(cx + dx, cy + dy, color);
                }
            }
        }
    }

    /// Draw a one-pixel-wide circle outline.
    pub fn ring(&mut self, cx: i32, cy: i32, r: i32, color: u8) {
        let r2 = r * r;
        let inner = (r - 1).max(0);
        let inner2 = inner * inner;
        for dy in -r..=r {
            for dx in -r..=r {
                let d2 = dx * dx + dy * dy;
                if d2 <= r2 && d2 >= inner2 {
                    self.put(cx + dx, cy + dy, color);
                }
            }
        }
    }

    /// Blit an indexed image. When `transparent` is `Some`, that index is
    /// skipped.
    pub fn blit(
        &mut self,
        src: &[u8],
        sw: usize,
        sh: usize,
        dx: i32,
        dy: i32,
        transparent: Option<u8>,
    ) {
        for y in 0..sh {
            let ty = dy + y as i32;
            if ty < 0 || ty as usize >= self.height {
                continue;
            }
            for x in 0..sw {
                let tx = dx + x as i32;
                if tx < 0 || tx as usize >= self.width {
                    continue;
                }
                let v = src[y * sw + x];
                if transparent == Some(v) {
                    continue;
                }
                self.pixels[ty as usize * self.width + tx as usize] = v;
            }
        }
    }

    /// Draw a font glyph (1-bit) at `(x, y)` in `color`.
    pub fn draw_glyph(
        &mut self,
        font: &crate::data::vga::Font,
        ch: u8,
        x: i32,
        y: i32,
        color: u8,
    ) {
        let Some((w, h)) = font.glyph(ch) else {
            return;
        };
        for gy in 0..h {
            for gx in 0..w {
                if font.glyph_pixel(ch, gx, gy).unwrap_or(false) {
                    self.put(x + gx as i32, y + gy as i32, color);
                }
            }
        }
    }

    /// Draw a NUL-terminated-ish string using a font.
    pub fn draw_text(&mut self, font: &crate::data::vga::Font, text: &str, x: i32, y: i32, color: u8) {
        let mut cx = x;
        for b in text.bytes() {
            if b == 0 {
                break;
            }
            self.draw_glyph(font, b, cx, y, color);
            cx += font.char_width(b) as i32;
        }
    }

    /// Convert the indexed image to RGBA8 (sRGB) for upload to the GPU.
    pub fn to_rgba(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.pixels.len() * 4);
        for &idx in &self.pixels {
            let [r, g, b] = palette::to_rgb(idx);
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
}
