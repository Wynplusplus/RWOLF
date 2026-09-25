//! A tiny built-in 3x5 bitmap font.
//!
//! This exists so the engine can show a diagnostic message *before* any game
//! data (and therefore the real Wolfenstein 3D font) is available — for
//! example the "no game data" screen on Android. It is deliberately minimal
//! and has no dependency on the WL6 files.
//!
//! SPDX-License-Identifier: MIT

use crate::render::framebuffer::Framebuffer;

/// Glyph height in pixels.
pub const GLYPH_H: i32 = 5;
/// Glyph width in pixels.
pub const GLYPH_W: i32 = 3;
/// Horizontal gap between glyphs (at scale 1).
pub const GLYPH_GAP: i32 = 1;

/// Rows are top-to-bottom; in each row bit 2 is the left pixel.
fn glyph(c: char) -> Option<[u8; 5]> {
    let g = match c.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b110, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b110, 0b101, 0b010],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b010, 0b101, 0b010, 0b101, 0b010],
        '9' => [0b010, 0b101, 0b011, 0b001, 0b110],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '!' => [0b010, 0b010, 0b010, 0b000, 0b010],
        '?' => [0b110, 0b001, 0b010, 0b000, 0b010],
        '(' => [0b010, 0b100, 0b100, 0b100, 0b010],
        ')' => [0b010, 0b001, 0b001, 0b001, 0b010],
        '\'' => [0b010, 0b010, 0b000, 0b000, 0b000],
        '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '=' => [0b000, 0b111, 0b000, 0b111, 0b000],
        _ => return None,
    };
    Some(g)
}

/// Width in pixels of `text` at the given scale.
pub fn text_width(text: &str, scale: i32) -> i32 {
    let n = text.chars().count() as i32;
    if n == 0 {
        0
    } else {
        n * (GLYPH_W + GLYPH_GAP) * scale - GLYPH_GAP * scale
    }
}

/// Draw `text` at `(x, y)` (top-left) with an integer `scale`.
pub fn draw_text(fb: &mut Framebuffer, text: &str, x: i32, y: i32, scale: i32, color: u8) {
    let scale = scale.max(1);
    let mut cx = x;
    for ch in text.chars() {
        if ch == ' ' {
            cx += (GLYPH_W + GLYPH_GAP) * scale;
            continue;
        }
        if let Some(rows) = glyph(ch) {
            for (ry, row) in rows.iter().enumerate() {
                for rx in 0..GLYPH_W {
                    if row & (1 << (GLYPH_W - 1 - rx)) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                fb.put(
                                    cx + rx * scale + sx,
                                    y + ry as i32 * scale + sy,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
        }
        cx += (GLYPH_W + GLYPH_GAP) * scale;
    }
}

/// Draw `text` horizontally centred in a 320-pixel-wide framebuffer.
pub fn draw_centered(fb: &mut Framebuffer, text: &str, y: i32, scale: i32, color: u8) {
    let x = (320 - text_width(text, scale)) / 2;
    draw_text(fb, text, x, y, scale, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_and_drawing() {
        assert_eq!(text_width("", 1), 0);
        assert_eq!(text_width("A", 1), 3);
        assert_eq!(text_width("AB", 1), 7);
        assert_eq!(text_width("AB", 2), 14);

        let mut fb = Framebuffer::new(320, 200);
        fb.clear(0);
        draw_centered(&mut fb, "NO GAME DATA", 10, 2, 0x0f);
        assert!(fb.pixels.iter().any(|&p| p != 0), "nothing was drawn");
    }
}
