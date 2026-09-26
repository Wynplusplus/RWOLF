//! Status-bar rendering, mirroring the original `DrawPlayScreen`.
//!
//! SPDX-License-Identifier: MIT

use crate::data::VgaData;
use crate::data::vga::{Font, pic};
use crate::game::hud::Hud;
use crate::render::framebuffer::{Framebuffer, STATUS_H, VIEW_H, VIEW_W};

/// Number of episodes/maps in the registered (`WL6`) release.
pub const EPISODES: usize = 6;
pub const EPISODE_MAPS: usize = 10;

/// Axis-aligned rectangle in framebuffer pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x as f32
            && y >= self.y as f32
            && x < (self.x + self.w) as f32
            && y < (self.y + self.h) as f32
    }
}

/// What a tap on the level-select overlay hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuHit {
    Episode(usize),
    Map(usize),
    Difficulty(usize),
    Start,
    Files,
    Back,
}

/// Difficulty labels shown on the level-select overlay.
pub const DIFFICULTIES: [&str; 4] = ["BABY", "EASY", "NORMAL", "HARD"];

pub fn episode_rect(index: usize) -> Rect {
    let (cw, ch, gap) = (30, 16, 6);
    let total = EPISODES as i32 * cw + (EPISODES as i32 - 1) * gap;
    let x0 = (VIEW_W as i32 - total) / 2;
    Rect {
        x: x0 + index as i32 * (cw + gap),
        y: 30,
        w: cw,
        h: ch,
    }
}

pub fn map_rect(index: usize) -> Rect {
    let (cw, ch, gap, cols) = (40, 20, 6, 5);
    let total = cols as i32 * cw + (cols as i32 - 1) * gap;
    let x0 = (VIEW_W as i32 - total) / 2;
    let col = (index % cols) as i32;
    let row = (index / cols) as i32;
    Rect {
        x: x0 + col * (cw + gap),
        y: 62 + row * (ch + gap),
        w: cw,
        h: ch,
    }
}

pub fn difficulty_rect(index: usize) -> Rect {
    let (cw, ch, gap) = (60, 16, 8);
    let total = 4 * cw + 3 * gap;
    let x0 = (VIEW_W as i32 - total) / 2;
    Rect {
        x: x0 + index as i32 * (cw + gap),
        y: 122,
        w: cw,
        h: ch,
    }
}

pub fn start_rect() -> Rect {
    Rect {
        x: 8,
        y: 164,
        w: 96,
        h: 22,
    }
}

pub fn files_rect() -> Rect {
    Rect {
        x: 112,
        y: 164,
        w: 96,
        h: 22,
    }
}

pub fn back_rect() -> Rect {
    Rect {
        x: 216,
        y: 164,
        w: 96,
        h: 22,
    }
}

/// Hit-test a framebuffer position against the level-select overlay.
pub fn menu_hit(x: f32, y: f32) -> Option<MenuHit> {
    for e in 0..EPISODES {
        if episode_rect(e).contains(x, y) {
            return Some(MenuHit::Episode(e));
        }
    }
    for m in 0..EPISODE_MAPS {
        if map_rect(m).contains(x, y) {
            return Some(MenuHit::Map(m));
        }
    }
    for d in 0..DIFFICULTIES.len() {
        if difficulty_rect(d).contains(x, y) {
            return Some(MenuHit::Difficulty(d));
        }
    }
    if start_rect().contains(x, y) {
        return Some(MenuHit::Start);
    }
    if files_rect().contains(x, y) {
        return Some(MenuHit::Files);
    }
    if back_rect().contains(x, y) {
        return Some(MenuHit::Back);
    }
    None
}

pub fn draw_status_bar(fb: &mut Framebuffer, vga: &VgaData, hud: &Hud) {
    let top = (VIEW_H - STATUS_H) as i32;
    if let Some(bar) = vga.pic(pic::STATUS_BAR) {
        fb.blit(&bar.pixels, bar.width, bar.height, 0, top, None);
    } else {
        fb.fill_rect(0, top, 320, STATUS_H as i32, 0x10);
    }

    // Face.
    let health = hud.health.clamp(0, 100);
    let face_index = if health > 0 {
        let base = (100 - health) / 16; // 0..6
        pic::FACE_1A + (base as usize) * 3 + (hud.face_frame % 3)
    } else {
        pic::FACE_8A
    };
    draw_pic(fb, vga, face_index, 17 * 8, top + 4);

    // Weapon icon.
    let weapon = pic::KNIFE + hud.weapon.min(3);
    draw_pic(fb, vga, weapon, 32 * 8, top + 8);

    // Keys.
    let gold = if hud.keys & 1 != 0 { pic::GOLD_KEY } else { pic::NO_KEY };
    let silver = if hud.keys & 2 != 0 { pic::SILVER_KEY } else { pic::NO_KEY };
    draw_pic(fb, vga, gold, 30 * 8, top + 4);
    draw_pic(fb, vga, silver, 30 * 8, top + 20);

    // Numbers (x is in 8-pixel latch units, y in pixels).
    latch_number(fb, vga, 2, top + 16, 2, hud.level as i64);
    latch_number(fb, vga, 6, top + 16, 6, hud.score as i64);
    latch_number(fb, vga, 14, top + 16, 1, hud.lives as i64);
    latch_number(fb, vga, 21, top + 16, 3, health as i64);
    latch_number(fb, vga, 27, top + 16, 2, hud.ammo as i64);
}

fn draw_pic(fb: &mut Framebuffer, vga: &VgaData, index: usize, x: i32, y: i32) {
    if let Some(p) = vga.pic(index) {
        fb.blit(&p.pixels, p.width, p.height, x, y, None);
    }
}

fn latch_number(fb: &mut Framebuffer, vga: &VgaData, x: i32, y: i32, width: i32, number: i64) {
    let text = number.to_string();
    let digits: Vec<u8> = text.bytes().collect();
    let mut cx = x;
    let mut w = width;
    // Right-justify with blanks, exactly like LatchNumber.
    let mut start = 0;
    while (digits.len() as i32) < w {
        draw_pic(fb, vga, pic::N_BLANK, cx * 8, y);
        cx += 1;
        w -= 1;
    }
    if (digits.len() as i32) > w {
        start = digits.len() - w as usize;
    }
    for &d in &digits[start..] {
        let idx = pic::N_0 + (d - b'0') as usize;
        draw_pic(fb, vga, idx, cx * 8, y);
        cx += 1;
    }
}

/// Data shown on the end-of-floor intermission.
pub struct Intermission {
    pub secret_floor: bool,
    pub time_secs: f32,
    pub par_secs: f32,
    pub kill: i32,
    pub secret: i32,
    pub treasure: i32,
    pub bonus: i32,
}

/// The `LevelCompleted` intermission: ratios, time and bonus over a blank
/// screen. The player continues with any key.
pub fn draw_intermission(fb: &mut Framebuffer, vga: &VgaData, info: &Intermission) {
    fb.fill_rect(0, 0, VIEW_W as i32, VIEW_H as i32, 0x00);
    let Some(font) = vga.font(0) else {
        return;
    };
    let title = if info.secret_floor {
        "SECRET FLOOR COMPLETED"
    } else {
        "FLOOR COMPLETED"
    };
    draw_centered(fb, font, title, 10, 0x0e);

    let mins = |s: f32| {
        let t = s.max(0.0) as i32;
        format!("{}:{:02}", t / 60, t % 60)
    };
    let time = format!("TIME     {}", mins(info.time_secs));
    let par = if info.par_secs > 0.0 {
        format!("PAR      {}", mins(info.par_secs))
    } else {
        "PAR      ??:??".to_string()
    };
    let kill = format!("KILL     {:>3}%", info.kill);
    let secret = format!("SECRET   {:>3}%", info.secret);
    let treasure = format!("TREASURE {:>3}%", info.treasure);
    let bonus = format!("BONUS    {}", info.bonus);

    let lines = [time, par, kill, secret, treasure, bonus];
    for (i, line) in lines.iter().enumerate() {
        draw_centered(fb, font, line, 36 + i as i32 * 16, 0x0f);
    }
    draw_centered(fb, font, "PRESS ANY KEY", 150, 0x0e);
}

/// Draw a centred message over the 3D view using the game's small font.
pub fn draw_center_text(fb: &mut Framebuffer, vga: &VgaData, text: &str, y: i32, color: u8) {
    let Some(font) = vga.font(0) else {
        return;
    };
    let width = text_width(font, text);
    let x = (VIEW_W as i32 - width) / 2;
    // A one-pixel shadow keeps the text readable on any background.
    fb.draw_text(font, text, x + 1, y + 1, 0);
    fb.draw_text(font, text, x, y, color);
}

fn text_width(font: &Font, text: &str) -> i32 {
    text.bytes().map(|b| font.char_width(b) as i32).sum()
}

fn draw_centered(fb: &mut Framebuffer, font: &Font, text: &str, y: i32, color: u8) {
    let x = (VIEW_W as i32 - text_width(font, text)) / 2;
    fb.draw_text(font, text, x, y, color);
}

/// A selectable box with a centred label. The active box is filled red and
/// outlined white; inactive boxes are dark grey.
fn draw_cell(
    fb: &mut Framebuffer,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    selected: bool,
) {
    let (bg, border) = if selected { (0x0c, 0x0f) } else { (0x08, 0x07) };
    fb.fill_rect(x, y, w, h, bg);
    fb.fill_rect(x, y, w, 1, border);
    fb.fill_rect(x, y + h - 1, w, 1, border);
    fb.fill_rect(x, y, 1, h, border);
    fb.fill_rect(x + w - 1, y, 1, h, border);
    let tx = x + (w - text_width(font, text)) / 2;
    let ty = y + (h - font.height as i32) / 2;
    fb.draw_text(font, text, tx, ty, 0x0f);
}

/// The level-select overlay, opened with `Escape`. Fills the whole screen so it
/// covers the frozen 3D view. `episode`/`map` are zero-based.
pub fn draw_level_select(
    fb: &mut Framebuffer,
    vga: &VgaData,
    episode: usize,
    map: usize,
    difficulty: usize,
) {
    fb.clear(0x00);
    let Some(font) = vga.font(0) else {
        return;
    };

    draw_centered(fb, font, "SELECT LEVEL", 6, 0x0e);

    // Episode row.
    draw_centered(fb, font, "EPISODE", 18, 0x0f);
    for e in 0..EPISODES {
        let r = episode_rect(e);
        draw_cell(fb, font, &(e + 1).to_string(), r.x, r.y, r.w, r.h, e == episode);
    }

    // Floor grid, five columns over two rows.
    draw_centered(fb, font, "FLOOR", 50, 0x0f);
    for m in 0..EPISODE_MAPS {
        let r = map_rect(m);
        draw_cell(fb, font, &(m + 1).to_string(), r.x, r.y, r.w, r.h, m == map);
    }

    // Difficulty row.
    draw_centered(fb, font, "DIFFICULTY", 110, 0x0f);
    for d in 0..DIFFICULTIES.len() {
        let r = difficulty_rect(d);
        draw_cell(fb, font, DIFFICULTIES[d], r.x, r.y, r.w, r.h, d == difficulty);
    }

    let summary = format!("EPISODE {} - FLOOR {}", episode + 1, map + 1);
    draw_centered(fb, font, &summary, 146, 0x0e);

    let start = start_rect();
    draw_cell(fb, font, "START", start.x, start.y, start.w, start.h, true);
    let files = files_rect();
    draw_cell(fb, font, "FILES", files.x, files.y, files.w, files.h, false);
    let back = back_rect();
    draw_cell(fb, font, "BACK", back.x, back.y, back.w, back.h, false);
}
