//! Rendering for the in-app game-folder picker.
//!
//! Uses the built-in 3x5 font so it works before any game data is loaded.
//!
//! SPDX-License-Identifier: MIT

use crate::data::EXT;
use crate::game::browser::{Browser, layout};
use crate::render::framebuffer::Framebuffer;
use crate::render::font;

const BG: u8 = 0x01;
const FG: u8 = 0x0f;
const DIM: u8 = 0x07;
const HILITE: u8 = 0x08;
const ACCENT: u8 = 0x0e;

pub fn draw(fb: &mut Framebuffer, browser: &Browser, can_close: bool) {
    fb.clear(BG);

    font::draw_centered(fb, "SELECT GAME FOLDER", 6, 2, FG);

    if can_close {
        button(fb, layout::close_rect(), "X", true);
    }

    // Current path, keeping the tail (the folder name) visible.
    let path = browser.cwd.display().to_string();
    font::draw_text(fb, &fit_tail(&path, 79), 4, 22, 1, DIM);

    // Status line.
    font::draw_text(fb, &fit_tail(&browser.status, 79), 4, 31, 1, ACCENT);

    if !browser.is_empty() {
        draw_list(fb, browser);
    } else if browser.has_game {
        font::draw_centered(fb, &format!("VSWAP.{EXT} IS HERE"), 80, 1, FG);
    } else {
        draw_help(fb);
    }

    draw_buttons(fb, browser);
}

fn draw_list(fb: &mut Framebuffer, browser: &Browser) {
    let visible = layout::VISIBLE_ROWS;
    for i in 0..visible {
        let index = browser.scroll + i;
        let Some(entry) = browser.entries.get(index) else {
            break;
        };
        let rect = layout::row_rect(i);
        let selected = index == browser.selected;
        if selected {
            fb.fill_rect(rect.0, rect.1, rect.2, rect.3 - 2, HILITE);
        }
        let marker = if selected { ">" } else { " " };
        let star = if entry.has_game { " *" } else { "" };
        let line = format!("{marker} {}{star}", entry.name);
        let color = if selected { FG } else { DIM };
        font::draw_text(fb, &fit_tail(&line, 71), rect.0 + 3, rect.1 + 2, 1, color);
    }

    // Scroll indicators.
    let up = layout::scroll_up_rect();
    let down = layout::scroll_down_rect();
    let can_up = browser.scroll > 0;
    let can_down = browser.scroll + visible < browser.entries.len();
    draw_arrow(fb, up, '^', can_up);
    draw_arrow(fb, down, 'v', can_down);
}

fn draw_help(fb: &mut Framebuffer) {
    // Nothing readable: explain how to grant access or where to put the data.
    let lines = [
        "THIS FOLDER IS EMPTY OR NOT READABLE.",
        "",
        "ON ANDROID 11+ ARWOLF NEEDS ALL FILES",
        "ACCESS TO READ SHARED STORAGE:",
        "  SETTINGS > APPS > ARWOLF > PERMISSIONS",
        "  > FILES AND MEDIA > ALLOW MANAGEMENT",
        "  OF ALL FILES.",
        "",
        "OR COPY YOUR WOLF3D FOLDER INTO THE",
        "APP FOLDER AND PRESS ROOTS:",
        "  /ANDROID/DATA/IO.GITHUB.WYNPLUSPLUS.",
        "  WOLF3DBEVY/FILES/",
    ];
    let mut y = 46;
    for line in lines {
        font::draw_text(fb, &fit_tail(line, 79), 6, y, 1, DIM);
        y += 8;
    }
}

fn draw_buttons(fb: &mut Framebuffer, browser: &Browser) {
    if browser.access_error {
        button(fb, layout::grant_rect(), "GRANT ACCESS", true);
    }
    button(fb, layout::up_rect(), "UP", true);
    button(fb, layout::roots_rect(), "ROOTS", true);
    let use_label = if browser.has_game {
        "USE THIS FOLDER"
    } else {
        "USE THIS FOLDER"
    };
    button(fb, layout::use_rect(), use_label, browser.has_game);
}

fn button(fb: &mut Framebuffer, rect: (i32, i32, i32, i32), label: &str, enabled: bool) {
    let (x, y, w, h) = rect;
    let fill = if enabled { HILITE } else { BG };
    fb.fill_rect(x, y, w, h, fill);
    let border = if enabled { FG } else { DIM };
    fb.fill_rect(x, y, w, 1, border);
    fb.fill_rect(x, y + h - 1, w, 1, border);
    fb.fill_rect(x, y, 1, h, border);
    fb.fill_rect(x + w - 1, y, 1, h, border);
    let color = if enabled { FG } else { DIM };
    let tw = font::text_width(label, 1);
    font::draw_text(fb, label, x + (w - tw) / 2, y + (h - 5) / 2, 1, color);
}

fn draw_arrow(fb: &mut Framebuffer, rect: (i32, i32, i32, i32), ch: char, enabled: bool) {
    let (x, y, w, h) = rect;
    let color = if enabled { FG } else { DIM };
    let s = ch.to_string();
    let tw = font::text_width(&s, 2);
    font::draw_text(fb, &s, x + (w - tw) / 2, y + (h - 10) / 2, 2, color);
}

/// Keep the end of a long string, prefixing `...` so the tail stays visible.
fn fit_tail(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let tail: String = s.chars().skip(count - keep).collect();
    format!("...{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_without_panicking() {
        let mut fb = Framebuffer::new(crate::render::framebuffer::VIEW_W, crate::render::framebuffer::VIEW_H);
        let browser = Browser::default();
        draw(&mut fb, &browser, false);
        draw(&mut fb, &browser, true);
    }

    #[test]
    fn fit_tail_shortens() {
        assert_eq!(fit_tail("abc", 5), "abc");
        assert_eq!(fit_tail("abcdef", 5), "...ef");
    }

    /// Render the picker at the repository root and dump it as a PPM.
    #[test]
    fn render_picker_ppm() {
        let mut browser = Browser::default();
        browser.open_at(std::env::current_dir().unwrap());
        let mut fb = Framebuffer::new(crate::render::framebuffer::VIEW_W, crate::render::framebuffer::VIEW_H);
        draw(&mut fb, &browser, false);
        let mut ppm = format!(
            "P6\n{} {}\n255\n",
            crate::render::framebuffer::VIEW_W,
            crate::render::framebuffer::VIEW_H
        )
        .into_bytes();
        for &idx in &fb.pixels {
            ppm.extend_from_slice(&crate::data::palette::to_rgb(idx));
        }
        std::fs::write(std::env::temp_dir().join("wolf3d_picker.ppm"), ppm).unwrap();

        // If the real data is present, render that folder too so the enabled
        // "USE THIS FOLDER" state is covered.
        if let Some(dir) = crate::data::find_data_dir() {
            let mut b2 = Browser::default();
            b2.open_at(dir);
            let mut fb2 = Framebuffer::new(crate::render::framebuffer::VIEW_W, crate::render::framebuffer::VIEW_H);
            draw(&mut fb2, &b2, true);
            let mut ppm = format!(
                "P6\n{} {}\n255\n",
                crate::render::framebuffer::VIEW_W,
                crate::render::framebuffer::VIEW_H
            )
            .into_bytes();
            for &idx in &fb2.pixels {
                ppm.extend_from_slice(&crate::data::palette::to_rgb(idx));
            }
            std::fs::write(std::env::temp_dir().join("wolf3d_picker_game.ppm"), ppm).unwrap();
        }
    }
}
