//! On-screen touch controls for phones and tablets (Android).
//!
//! Touches are reported in logical window pixels. The game is drawn into a
//! 320x200 framebuffer that Bevy letterboxes into the window, so touches are
//! mapped back into framebuffer coordinates using the same aspect-preserving
//! rule as the camera (`ScalingMode::AutoMin`). The buttons are then defined
//! and drawn entirely in framebuffer space, which keeps the controls and the
//! game pixels in one coordinate system and makes the hit boxes exact.
//!
//! Layout (framebuffer coordinates):
//!
//! * left half  — virtual movement stick: up/down moves, left/right turns
//! * right half — drag to turn
//! * top row    — MENU, four weapon buttons, RUN
//! * bottom right — FIRE and USE
//!
//! SPDX-License-Identifier: MIT

use std::collections::HashMap;

use bevy::input::touch::Touches;
use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::data::VgaData;
use crate::render::framebuffer::{Framebuffer, VIEW_3D_H, VIEW_W};

const STICK_RADIUS: f32 = 34.0;
const STICK_DEADZONE: f32 = 0.16;
/// Turn per framebuffer pixel of drag. Larger than the mouse because a finger
/// swipe is shorter than a mouse movement.
const LOOK_SENSITIVITY: f32 = 2.4;

const MENU_CENTER: (f32, f32) = (30.0, 18.0);
const MENU_RADIUS: f32 = 15.0;
const RUN_CENTER: (f32, f32) = (290.0, 18.0);
const RUN_RADIUS: f32 = 15.0;
const WEAPON_Y: f32 = 18.0;
const WEAPON_RADIUS: f32 = 11.0;
const WEAPON_X: [f32; 4] = [140.0, 166.0, 192.0, 218.0];
const FIRE_CENTER: (f32, f32) = (292.0, 140.0);
const FIRE_RADIUS: f32 = 22.0;
const USE_CENTER: (f32, f32) = (248.0, 148.0);
const USE_RADIUS: f32 = 15.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Move,
    Look,
    Fire,
    Use,
    Run,
    Menu,
    Weapon(usize),
}

/// Virtual gamepad state for the current frame.
#[derive(Resource)]
pub struct TouchControls {
    /// Whether the overlay is active at all. True on Android (or when
    /// `WOLF3D_TOUCH` is set for testing on the desktop).
    pub enabled: bool,
    /// `x` = turn (-1..1), `y` = forward (-1..1).
    pub movement: Vec2,
    /// Extra turning accumulated from a look drag, in mouse-delta units.
    pub look_dx: f32,
    pub fire: bool,
    pub fire_pressed: bool,
    pub use_pressed: bool,
    pub run: bool,
    /// Set for one frame when the MENU button is tapped.
    pub menu_pressed: bool,
    /// Set for one frame when a weapon button is tapped.
    pub weapon: Option<usize>,
    /// A tap in framebuffer coordinates, for the level-select overlay.
    pub tap: Option<Vec2>,

    roles: HashMap<u64, Role>,
    stick_origin: Option<Vec2>,
    stick_current: Option<Vec2>,
}

impl Default for TouchControls {
    fn default() -> Self {
        let enabled = cfg!(target_os = "android")
            || std::env::var("WOLF3D_TOUCH")
                .map(|v| !v.is_empty() && v != "0")
                .unwrap_or(false);
        Self {
            enabled,
            movement: Vec2::ZERO,
            look_dx: 0.0,
            fire: false,
            fire_pressed: false,
            use_pressed: false,
            run: false,
            menu_pressed: false,
            weapon: None,
            tap: None,
            roles: HashMap::new(),
            stick_origin: None,
            stick_current: None,
        }
    }
}

/// The letterbox mapping between framebuffer pixels and window pixels.
struct Viewport {
    scale: f32,
    offset_x: f32,
    offset_y: f32,
}

impl Viewport {
    fn new(window: Vec2) -> Option<Self> {
        if window.x <= 0.0 || window.y <= 0.0 {
            return None;
        }
        let scale = (window.x / VIEW_W as f32).min(window.y / 200.0);
        if scale <= 0.0 {
            return None;
        }
        Some(Self {
            scale,
            offset_x: (window.x - VIEW_W as f32 * scale) * 0.5,
            offset_y: (window.y - 200.0 * scale) * 0.5,
        })
    }

    /// Window pixels -> framebuffer pixels.
    fn to_fb(&self, p: Vec2) -> Vec2 {
        Vec2::new(
            (p.x - self.offset_x) / self.scale,
            (p.y - self.offset_y) / self.scale,
        )
    }

    /// A length in window pixels -> framebuffer pixels.
    fn to_fb_len(&self, v: f32) -> f32 {
        v / self.scale
    }
}

fn hit(p: Vec2, center: (f32, f32), radius: f32) -> bool {
    (p - Vec2::new(center.0, center.1)).length_squared() <= radius * radius
}

/// Reset the controls and read the active touches.
pub fn read_touch(
    touches: Res<Touches>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut controls: ResMut<TouchControls>,
) {
    let c = &mut *controls;
    c.movement = Vec2::ZERO;
    c.look_dx = 0.0;
    c.fire = false;
    c.fire_pressed = false;
    c.use_pressed = false;
    c.run = false;
    c.menu_pressed = false;
    c.weapon = None;
    c.tap = None;

    if !c.enabled {
        return;
    }
    let Ok(window) = window.single() else {
        return;
    };
    let Some(vp) = Viewport::new(Vec2::new(window.width(), window.height())) else {
        c.roles.clear();
        c.stick_origin = None;
        c.stick_current = None;
        return;
    };

    // Drop roles for fingers that have lifted.
    let live: Vec<u64> = touches.iter().map(|t| t.id()).collect();
    c.roles.retain(|id, _| live.contains(id));

    let mut any = false;
    for touch in touches.iter() {
        any = true;
        let id = touch.id();
        let start = vp.to_fb(touch.start_position());
        let pos = vp.to_fb(touch.position());
        let just = touches.just_pressed(id);
        if just {
            c.tap = Some(pos);
        }

        let role = *c.roles.entry(id).or_insert_with(|| {
            // Decide the role from where the finger first landed. Buttons win
            // over the look area.
            for (i, &x) in WEAPON_X.iter().enumerate() {
                if hit(start, (x, WEAPON_Y), WEAPON_RADIUS + 6.0) {
                    return Role::Weapon(i);
                }
            }
            if hit(start, MENU_CENTER, MENU_RADIUS + 6.0) {
                return Role::Menu;
            }
            if hit(start, RUN_CENTER, RUN_RADIUS + 6.0) {
                return Role::Run;
            }
            if hit(start, FIRE_CENTER, FIRE_RADIUS + 6.0) {
                return Role::Fire;
            }
            if hit(start, USE_CENTER, USE_RADIUS + 6.0) {
                return Role::Use;
            }
            if start.x < VIEW_W as f32 * 0.5 {
                Role::Move
            } else {
                Role::Look
            }
        });

        match role {
            Role::Move => {
                if c.stick_origin.is_none() {
                    c.stick_origin = Some(start);
                }
                c.stick_current = Some(pos);
                let origin = c.stick_origin.unwrap_or(start);
                let mut v = (pos - origin) / STICK_RADIUS;
                v.x = v.x.clamp(-1.0, 1.0);
                v.y = v.y.clamp(-1.0, 1.0);
                if v.length() < STICK_DEADZONE {
                    v = Vec2::ZERO;
                }
                // Stick x turns, stick y moves forward (screen y is down).
                c.movement.x += v.x;
                c.movement.y += -v.y;
            }
            Role::Look => {
                c.look_dx += vp.to_fb_len(touch.delta().x) * LOOK_SENSITIVITY;
            }
            Role::Fire => {
                c.fire = true;
                if just {
                    c.fire_pressed = true;
                }
            }
            Role::Use => {
                if just {
                    c.use_pressed = true;
                }
            }
            Role::Run => c.run = true,
            Role::Menu => {
                if just {
                    c.menu_pressed = true;
                }
            }
            Role::Weapon(i) => {
                if just {
                    c.weapon = Some(i);
                }
            }
        }
    }

    if !any {
        c.stick_origin = None;
        c.stick_current = None;
    }
    c.movement.x = c.movement.x.clamp(-1.0, 1.0);
    c.movement.y = c.movement.y.clamp(-1.0, 1.0);
}

fn text_width(font: &crate::data::vga::Font, text: &str) -> i32 {
    text.bytes().map(|b| font.char_width(b) as i32).sum()
}

fn label(fb: &mut Framebuffer, font: &crate::data::vga::Font, text: &str, cx: i32, cy: i32) {
    let w = text_width(font, text);
    let x = cx - w / 2;
    let y = cy - font.height as i32 / 2;
    // Shadow for readability over any background.
    fb.draw_text(font, text, x + 1, y + 1, 0x00);
    fb.draw_text(font, text, x, y, 0x0f);
}

/// Draw the virtual gamepad over the 3D view.
pub fn draw_controls(fb: &mut Framebuffer, vga: &VgaData, controls: &TouchControls) {
    if !controls.enabled {
        return;
    }
    let Some(font) = vga.font(0) else {
        return;
    };

    const FILL: u8 = 0x08;
    const OUTLINE: u8 = 0x0f;
    const ACTIVE: u8 = 0x04;

    let button = |fb: &mut Framebuffer, center: (f32, f32), r: f32, pressed: bool| {
        fb.circle(
            center.0 as i32,
            center.1 as i32,
            r as i32,
            if pressed { ACTIVE } else { FILL },
        );
        fb.ring(center.0 as i32, center.1 as i32, r as i32, OUTLINE);
    };

    button(fb, MENU_CENTER, MENU_RADIUS, controls.menu_pressed);
    label(fb, font, "MENU", MENU_CENTER.0 as i32, MENU_CENTER.1 as i32);

    button(fb, RUN_CENTER, RUN_RADIUS, controls.run);
    label(fb, font, "RUN", RUN_CENTER.0 as i32, RUN_CENTER.1 as i32);

    for (i, &x) in WEAPON_X.iter().enumerate() {
        button(fb, (x, WEAPON_Y), WEAPON_RADIUS, controls.weapon == Some(i));
        label(fb, font, &(i + 1).to_string(), x as i32, WEAPON_Y as i32);
    }

    button(fb, FIRE_CENTER, FIRE_RADIUS, controls.fire);
    label(fb, font, "FIRE", FIRE_CENTER.0 as i32, FIRE_CENTER.1 as i32);

    button(fb, USE_CENTER, USE_RADIUS, controls.use_pressed);
    label(fb, font, "USE", USE_CENTER.0 as i32, USE_CENTER.1 as i32);

    // Movement stick.
    match controls.stick_origin {
        Some(origin) => {
            fb.ring(origin.x as i32, origin.y as i32, STICK_RADIUS as i32, OUTLINE);
            if let Some(cur) = controls.stick_current {
                fb.circle(cur.x as i32, cur.y as i32, 9, OUTLINE);
            }
        }
        None => {
            // Faint hint where the stick appears when you touch.
            fb.ring(56, VIEW_3D_H as i32 - 44, STICK_RADIUS as i32, FILL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_maps_the_centre() {
        let vp = Viewport::new(Vec2::new(640.0, 400.0)).unwrap();
        let p = vp.to_fb(Vec2::new(320.0, 200.0));
        assert!((p.x - 160.0).abs() < 0.01);
        assert!((p.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn viewport_letterboxes_wide_windows() {
        // 800x400 is wider than 1.6, so there are bars on the sides.
        let vp = Viewport::new(Vec2::new(800.0, 400.0)).unwrap();
        assert!((vp.scale - 2.0).abs() < 0.001);
        assert!((vp.offset_x - 80.0).abs() < 0.001);
        assert!(vp.offset_y.abs() < 0.001);
    }

    #[test]
    fn hit_testing() {
        assert!(hit(Vec2::new(10.0, 10.0), (10.0, 10.0), 5.0));
        assert!(!hit(Vec2::new(20.0, 10.0), (10.0, 10.0), 5.0));
    }
}
