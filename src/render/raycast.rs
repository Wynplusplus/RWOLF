//! Software raycaster reproducing the original's projection.
//!
//! Wolfenstein 3D uses a 320x160 3D viewport (the remaining 40 rows are the
//! status bar). The camera plane half-width and vertical projection constants
//! below are derived from the original `CalcProjection`:
//!
//! * `facedist = FOCALLENGTH + MINDIST = 0x5700 + 0x5800 = 44800`
//! * `plane_len = (VIEWGLOBAL / 2) / facedist = 32768 / 44800`
//! * `proj_h = (view_width / 2) / plane_len`, `proj_v = proj_h / 2`
//!
//! SPDX-License-Identifier: MIT

use crate::data::VSwap;
use crate::game::actor::Actor;
use crate::game::level::Level;
use crate::render::framebuffer::{Framebuffer, VIEW_3D_H, VIEW_W};

pub const PLANE_LEN: f32 = 32768.0 / 44800.0;
pub const PROJ_H: f32 = (VIEW_W as f32 / 2.0) / PLANE_LEN;
pub const PROJ_V: f32 = PROJ_H / 2.0;
pub const HORIZON: f32 = VIEW_3D_H as f32 / 2.0;

/// `FOCALLENGTH = 0x5700`, in tiles. The original positions the camera this
/// distance behind the player (`viewx = x - focal·cos`), so every depth used
/// for projection is `distance + FOCAL`.
pub const FOCAL: f32 = 0x5700 as f32 / 65536.0;

/// `ACTORSIZE = 0x4000`: actors are nudged this much closer so their midpoint
/// does not clip into an adjacent wall.
pub const ACTOR_FUDGE: f32 = 0x4000 as f32 / 65536.0;
/// `0x2000`: the same nudge for static objects (`TransformTile`).
pub const STATIC_FUDGE: f32 = 0x2000 as f32 / 65536.0;

/// Door texture chunks (relative to `DOORWALL = sprite_start - 8`).
const DOOR_NORMAL: usize = 0;
const DOOR_SIDE_H: usize = 2;
const DOOR_SIDE_V: usize = 3;
const DOOR_ELEVATOR: usize = 4;
const DOOR_LOCKED: usize = 6;

#[derive(Clone, Copy)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub angle: f32,
}

impl Camera {
    #[inline]
    pub fn dir(&self) -> (f32, f32) {
        (self.angle.cos(), -self.angle.sin())
    }
    #[inline]
    pub fn plane(&self) -> (f32, f32) {
        let (dx, dy) = self.dir();
        (-dy * PLANE_LEN, dx * PLANE_LEN)
    }

    /// The camera position, offset `FOCAL` behind the player along the view
    /// direction (matching the original's `viewx`/`viewy`).
    #[inline]
    pub fn origin(&self) -> (f32, f32) {
        let (dx, dy) = self.dir();
        (self.x - dx * FOCAL, self.y - dy * FOCAL)
    }
}

struct Hit {
    perp: f32,
    texture: usize,
    tex_u: f32,
    /// Fractional position within the door for sliding textures.
    door_offset: Option<f32>,
}

/// Render the 3D view into `fb` rows `0..VIEW_3D_H`, and fill `zbuf` with the
/// perpendicular wall distance per column.
pub fn render_walls(
    fb: &mut Framebuffer,
    vswap: &VSwap,
    level: &Level,
    cam: Camera,
    zbuf: &mut [f32; VIEW_W],
) {
    // Ceiling above the horizon, floor below (matches VGAClearScreen).
    fb.fill_rect(0, 0, VIEW_W as i32, HORIZON as i32, level.ceiling_color);
    fb.fill_rect(
        0,
        HORIZON as i32,
        VIEW_W as i32,
        VIEW_3D_H as i32 - HORIZON as i32,
        level.floor_color,
    );

    let (dx, dy) = cam.dir();
    let (px_, py_) = cam.plane();
    // Rays originate `FOCAL` behind the player, like the original.
    let cam_at = Camera {
        x: cam.x - dx * FOCAL,
        y: cam.y - dy * FOCAL,
        angle: cam.angle,
    };
    let sprite_start = vswap.sprite_start as usize;
    let door_base = sprite_start.saturating_sub(8);

    for x in 0..VIEW_W {
        let camera_x = 2.0 * x as f32 / VIEW_W as f32 - 1.0;
        let rdx = dx + px_ * camera_x;
        let rdy = dy + py_ * camera_x;
        let hit = cast(level, cam_at, rdx, rdy, door_base, sprite_start);
        let Some(hit) = hit else {
            zbuf[x] = f32::INFINITY;
            continue;
        };
        zbuf[x] = hit.perp;
        let line_h = PROJ_V / hit.perp;
        let draw_start = (HORIZON - line_h / 2.0).round() as i32;
        let draw_end = (HORIZON + line_h / 2.0).round() as i32;
        let top = draw_start.max(0);
        let bottom = draw_end.min(VIEW_3D_H as i32);

        let Some(tex) = vswap.wall(hit.texture) else {
            continue;
        };
        let span = (draw_end - draw_start).max(1) as f32;
        let u = hit.tex_u.clamp(0.0, 0.999_999);
        let mut tex_x = (u * 64.0) as usize;
        if tex_x > 63 {
            tex_x = 63;
        }
        // Slide the door texture as it opens.
        if let Some(off) = hit.door_offset {
            let mut uu = (u - off).rem_euclid(1.0);
            if uu >= 1.0 {
                uu -= 1.0;
            }
            tex_x = ((uu * 64.0) as usize).min(63);
        }
        for y in top..bottom {
            let tex_y = (((y as f32 - draw_start as f32) / span) * 64.0) as i32;
            let tex_y = tex_y.clamp(0, 63) as usize;
            let color = tex[tex_y * 64 + tex_x];
            fb.put(x as i32, y, color);
        }
    }
}

fn cast(
    level: &Level,
    cam: Camera,
    rdx: f32,
    rdy: f32,
    door_base: usize,
    _sprite_start: usize,
) -> Option<Hit> {
    let mut map_x = cam.x.floor() as i32;
    let mut map_y = cam.y.floor() as i32;
    let delta_x = if rdx.abs() < 1e-9 { f32::INFINITY } else { (1.0 / rdx).abs() };
    let delta_y = if rdy.abs() < 1e-9 { f32::INFINITY } else { (1.0 / rdy).abs() };
    let (step_x, side_dist_x) = if rdx < 0.0 {
        (-1, (cam.x - map_x as f32) * delta_x)
    } else {
        (1, (map_x as f32 + 1.0 - cam.x) * delta_x)
    };
    let (step_y, side_dist_y) = if rdy < 0.0 {
        (-1, (cam.y - map_y as f32) * delta_y)
    } else {
        (1, (map_y as f32 + 1.0 - cam.y) * delta_y)
    };

    let mut side_dist_x = side_dist_x;
    let mut side_dist_y = side_dist_y;

    for _ in 0..(level.width + level.height) * 2 + 16 {
        let side;
        let mut perp;
        if side_dist_x < side_dist_y {
            perp = side_dist_x;
            side_dist_x += delta_x;
            map_x += step_x;
            side = 0;
        } else {
            perp = side_dist_y;
            side_dist_y += delta_y;
            map_y += step_y;
            side = 1;
        }
        if map_x < 0 || map_y < 0 || map_x as usize >= level.width || map_y as usize >= level.height {
            return None;
        }
        let tile = level.tile(map_x as usize, map_y as usize);
        if tile == 0 {
            continue;
        }

        // Door?
        if tile & 0x80 != 0 {
            let doornum = (tile & 0x7F) as usize;
            let Some(door) = level.doors.get(doornum) else {
                continue;
            };
            let (plane_t, frac) = if door.vertical {
                // Plane at x = map_x + 0.5.
                if rdx.abs() < 1e-9 {
                    continue;
                }
                let t = (map_x as f32 + 0.5 - cam.x) / rdx;
                let y = cam.y + rdy * t;
                (t, y - map_y as f32)
            } else {
                if rdy.abs() < 1e-9 {
                    continue;
                }
                let t = (map_y as f32 + 0.5 - cam.y) / rdy;
                let xx = cam.x + rdx * t;
                (t, xx - map_x as f32)
            };
            if plane_t <= perp {
                // Door plane is behind the entry point; treat as pass-through.
                continue;
            }
            if frac < door.position {
                // The door has slid clear of this part of the tile.
                continue;
            }
            perp = plane_t;
            let lock = door.lock;
            let base = if lock == 5 {
                DOOR_ELEVATOR
            } else if lock >= 1 {
                DOOR_LOCKED
            } else {
                DOOR_NORMAL
            };
            let texture = door_base + base + if door.vertical { 1 } else { 0 };
            return Some(Hit {
                perp,
                texture,
                tex_u: frac.rem_euclid(1.0),
                door_offset: Some(door.position),
            });
        }

        // Solid wall. `0x40` marks a tile next to a door. Such a tile can be a
        // plain floor tile at the end of a wall; the original still draws the
        // door frame on its face towards the door, so the door-side test has to
        // run before the `base == 0` bail-out.
        let base = (tile & 0x3F) as usize;
        let mut door_side = None;
        if tile & 0x40 != 0 {
            let (nx, ny) = if side == 0 {
                (map_x - step_x, map_y)
            } else {
                (map_x, map_y - step_y)
            };
            if nx >= 0 && ny >= 0 && (nx as usize) < level.width && (ny as usize) < level.height
                && level.tile(nx as usize, ny as usize) & 0x80 != 0
            {
                door_side = Some(door_base + if side == 0 { DOOR_SIDE_V } else { DOOR_SIDE_H });
            }
        }
        let texture = match door_side {
            Some(t) => t,
            None => {
                if base == 0 {
                    continue;
                }
                if side == 0 {
                    (base - 1) * 2 + 1
                } else {
                    (base - 1) * 2
                }
            }
        };
        let wall_x = if side == 0 {
            cam.y + perp * rdy
        } else {
            cam.x + perp * rdx
        };
        let mut frac = wall_x - wall_x.floor();
        // Mirror textures exactly like the original's `xtilestep`/`ytilestep`
        // handling so adjacent faces meet consistently.
        if (side == 0 && rdx < 0.0) || (side == 1 && rdy > 0.0) {
            frac = 1.0 - frac;
        }
        return Some(Hit {
            perp,
            texture,
            tex_u: frac,
            door_offset: None,
        });
    }
    None
}

/// A sprite to draw in the 3D view.
pub struct SpriteInstance {
    pub x: f32,
    pub y: f32,
    pub sprite: u16,
    /// Nudge toward the viewer (`ACTORSIZE`/`0x2000` in the original).
    pub fudge: f32,
}

/// Draw statics and actors back-to-front, clipped by the wall depth buffer.
pub fn render_sprites(
    fb: &mut Framebuffer,
    vswap: &VSwap,
    cam: Camera,
    zbuf: &[f32; VIEW_W],
    sprites: &[SpriteInstance],
) {
    let (dx, dy) = cam.dir();
    let (px_, py_) = cam.plane();
    let (ox, oy) = cam.origin();
    let det = px_ * dy - dx * py_;
    if det.abs() < 1e-9 {
        return;
    }
    let inv_det = 1.0 / det;

    let mut order: Vec<(f32, &SpriteInstance)> = sprites
        .iter()
        .map(|s| {
            let rx = s.x - ox;
            let ry = s.y - oy;
            let depth = inv_det * (-py_ * rx + px_ * ry);
            (depth, s)
        })
        .filter(|(d, _)| *d > 0.05)
        .collect();
    order.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    for (depth, s) in order {
        let rx = s.x - ox;
        let ry = s.y - oy;
        let transform_x = inv_det * (dy * rx - dx * ry);
        let proj_depth = (depth - s.fudge).max(0.05);
        let screen_x = (VIEW_W as f32 / 2.0) * (1.0 + transform_x / proj_depth);
        let size = PROJ_V / proj_depth;
        if size < 1.0 {
            continue;
        }
        let half = size / 2.0;
        let left = screen_x - half;
        let top = HORIZON - half;
        let Some(sprite) = vswap.sprite(s.sprite as usize) else {
            continue;
        };

        let x_start = left.floor().max(0.0) as i32;
        let x_end = (left + size).ceil().min(VIEW_W as f32) as i32;
        let y_start = top.max(0.0) as i32;
        let y_end = (top + size).min(VIEW_3D_H as f32) as i32;
        if x_end <= x_start || y_end <= y_start {
            continue;
        }
        for x in x_start..x_end {
            if zbuf[x as usize] < proj_depth {
                continue;
            }
            let tex_x = (((x as f32 - left) / size) * 64.0) as i32;
            if !(0..64).contains(&tex_x) {
                continue;
            }
            for y in y_start..y_end {
                let tex_y = (((y as f32 - top) / size) * 64.0) as i32;
                if !(0..64).contains(&tex_y) {
                    continue;
                }
                if let Some(color) = sprite.at(tex_x as usize, tex_y as usize) {
                    fb.put(x, y, color);
                }
            }
        }
    }
}

/// Draw the player's weapon the way the original does. `DrawPlayerWeapon`
/// calls `SimpleScaleShape(viewwidth/2, spr, viewheight+1)`; that height is
/// halved to index the compiled-scaler table, which resolves to a scale of
/// exactly `viewheight` anchored at the top of the viewport. The weapon art
/// (which lives in the lower part of the 64x64 shape) therefore rests on the
/// bottom edge of the view instead of floating mid-screen.
pub fn render_weapon(fb: &mut Framebuffer, vswap: &VSwap, sprite: u16) {
    let Some(spr) = vswap.sprite(sprite as usize) else {
        return;
    };
    let size = VIEW_3D_H as f32;
    let left = VIEW_W as f32 / 2.0 - size / 2.0;
    let top = 0.0f32;
    let x_start = left.floor().max(0.0) as i32;
    let x_end = (left + size).ceil().min(VIEW_W as f32) as i32;
    let y_start = top.max(0.0) as i32;
    let y_end = (top + size).min(VIEW_3D_H as f32) as i32;
    for x in x_start..x_end {
        let tex_x = (((x as f32 - left) / size) * 64.0) as i32;
        if !(0..64).contains(&tex_x) {
            continue;
        }
        for y in y_start..y_end {
            let tex_y = (((y as f32 - top) / size) * 64.0) as i32;
            if !(0..64).contains(&tex_y) {
                continue;
            }
            if let Some(color) = spr.at(tex_x as usize, tex_y as usize) {
                fb.put(x, y, color);
            }
        }
    }
}

/// Convenience: build the sprite list from statics and actors.
pub fn collect_sprites(level: &Level, actors: &[Actor], view_angle: f32) -> Vec<SpriteInstance> {
    let mut out: Vec<SpriteInstance> = Vec::with_capacity(level.statics.len() + actors.len());
    for s in &level.statics {
        if s.active {
            out.push(SpriteInstance {
                x: s.x as f32 + 0.5,
                y: s.y as f32 + 0.5,
                sprite: s.sprite,
                fudge: STATIC_FUDGE,
            });
        }
    }
    for a in actors {
        if let Some(sprite) = a.sprite(view_angle) {
            out.push(SpriteInstance {
                x: a.x,
                y: a.y,
                sprite,
                fudge: ACTOR_FUDGE,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::game::actor::Difficulty;
    use crate::game::world::World;

    /// Every door's inner frame face should select a door-side texture.
    #[test]
    fn door_side_faces_use_side_textures() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let sprite_start = data.vswap.sprite_start as usize;
        let door_base = sprite_start.saturating_sub(8);
        let mut bad = 0usize;
        let mut total = 0usize;
        for mi in 0..60 {
            let Some(_map) = data.maps.get(mi) else { continue };
            let mut world = World::new(&data, mi / 10, mi % 10, Difficulty::Normal).unwrap();
            for d in world.level.doors.iter_mut() {
                d.position = 1.0;
            }
            let doors = world.level.doors.clone();
            for (i, d) in doors.iter().enumerate() {
                let (cx, cy) = (d.x as f32 + 0.5, d.y as f32 + 0.5);
                let (rays, want) = if d.vertical {
                    (
                        [(0.0f32, -1.0f32), (0.0, 1.0)],
                        door_base + DOOR_SIDE_H,
                    )
                } else {
                    ([(1.0f32, 0.0f32), (-1.0, 0.0)], door_base + DOOR_SIDE_V)
                };
                for (rdx, rdy) in rays {
                    total += 1;
                    let cam = Camera {
                        x: cx,
                        y: cy,
                        angle: 0.0,
                    };
                    match cast(&world.level, cam, rdx, rdy, door_base, sprite_start) {
                        Some(h) if h.texture == want => {}
                        Some(h) => {
                            bad += 1;
                            if bad <= 20 {
                                eprintln!(
                                    "map {mi} door {i} vert={} tex={} want={want}",
                                    d.vertical, h.texture
                                );
                            }
                        }
                        None => {
                            bad += 1;
                            if bad <= 20 {
                                eprintln!("map {mi} door {i} vert={} NO HIT want={want}", d.vertical);
                            }
                        }
                    }
                }
            }
        }
        eprintln!("checked {total} door faces, {bad} wrong");
        assert_eq!(bad, 0, "{bad}/{total} door frame faces wrong");
    }
}
