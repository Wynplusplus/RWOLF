//! A self-contained diagnostic screen shown when the game cannot start
//! (for example when no WL6 data is present).
//!
//! It uses the built-in 3x5 font from [`crate::render::font`], so it works
//! before any game data is loaded.
//!
//! SPDX-License-Identifier: MIT

use crate::render::framebuffer::{Framebuffer, VIEW_H, VIEW_W};
use crate::render::font;

/// Draw a centred title and body text on a dark background.
pub fn draw(fb: &mut Framebuffer, title: &str, lines: &[String]) {
    fb.clear(0x01); // dark blue

    // A thin border so the screen does not look broken.
    let border = 0x08;
    fb.fill_rect(0, 0, VIEW_W as i32, 2, border);
    fb.fill_rect(0, VIEW_H as i32 - 2, VIEW_W as i32, 2, border);
    fb.fill_rect(0, 0, 2, VIEW_H as i32, border);
    fb.fill_rect(VIEW_W as i32 - 2, 0, 2, VIEW_H as i32, border);

    font::draw_centered(fb, title, 28, 3, 0x0f);

    let mut y = 62;
    for line in lines {
        if line.is_empty() {
            y += 10;
            continue;
        }
        font::draw_centered(fb, line, y, 2, 0x0f);
        y += 16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_something() {
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        draw(
            &mut fb,
            "NO GAME DATA",
            &["PUSH YOUR WOLF3D FILES.".to_string()],
        );
        let lit = fb.pixels.iter().filter(|&&p| p == 0x0f).count();
        assert!(lit > 50, "expected text pixels, got {lit}");
    }
}
