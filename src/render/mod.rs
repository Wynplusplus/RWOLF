//! The software renderer.
//!
//! SPDX-License-Identifier: MIT

pub mod browser_ui;
pub mod error_screen;
pub mod font;
pub mod framebuffer;
pub mod hud;
pub mod raycast;

#[cfg(test)]
mod tests {
    use super::framebuffer::{Framebuffer, VIEW_H, VIEW_W};
    use super::hud::draw_status_bar;
    use super::raycast::{
        Camera, PROJ_V, FOCAL, collect_sprites, render_sprites, render_walls, render_weapon,
    };
    use crate::data::palette;
    use crate::data::GameData;
    use crate::game::actor::Difficulty;
    use crate::game::level::build_level;
    use crate::game::world::{InputState, World};
    use crate::data::Map;

    /// Render the first frame of E1M1 and dump it as a PPM for visual
    /// inspection. The test is a no-op when no data directory is available.
    #[test]
    fn render_e1m1_frame() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let world = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        let cam = Camera {
            x: world.player.x,
            y: world.player.y,
            angle: world.player.angle,
        };
        let mut zbuf = [f32::INFINITY; VIEW_W];
        render_walls(&mut fb, &data.vswap, &world.level, cam, &mut zbuf);
        let sprites = collect_sprites(&world.level, &world.actors, world.player.angle);
        render_sprites(&mut fb, &data.vswap, cam, &zbuf, &sprites);
        render_weapon(&mut fb, &data.vswap, world.player.weapon_sprite());
        draw_status_bar(&mut fb, &data.vga, &world.hud);

        let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
        for &idx in &fb.pixels {
            ppm.extend_from_slice(&palette::to_rgb(idx));
        }
        let out = std::env::temp_dir().join("wolf3d_frame.ppm");
        std::fs::write(&out, ppm).unwrap();
        eprintln!("wrote {}", out.display());
    }

    /// Render the starting area from four headings for visual inspection.
    #[test]
    fn render_e1m1_headings() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let base = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        for (n, angle) in [0.0f32, 1.5708, 3.1416, 4.7124].into_iter().enumerate() {
            let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
            let cam = Camera {
                x: base.player.x,
                y: base.player.y,
                angle,
            };
            let mut zbuf = [f32::INFINITY; VIEW_W];
            render_walls(&mut fb, &data.vswap, &base.level, cam, &mut zbuf);
            let sprites = collect_sprites(&base.level, &base.actors, angle);
            render_sprites(&mut fb, &data.vswap, cam, &zbuf, &sprites);
            draw_status_bar(&mut fb, &data.vga, &base.hud);
            let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
            for &idx in &fb.pixels {
                ppm.extend_from_slice(&palette::to_rgb(idx));
            }
            let out = std::env::temp_dir().join(format!("wolf3d_heading{n}.ppm"));
            std::fs::write(&out, ppm).unwrap();
        }
        eprintln!("wrote headings");
    }

    /// Look at the first enemy in E1M1 to verify sprite rendering.
    #[test]
    fn render_e1m1_enemy() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let base = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        eprintln!(
            "E1M1 statics={} actors={}",
            base.level.statics.len(),
            base.actors.len()
        );
        let Some(a) = base.actors.first() else {
            return;
        };
        // Stand two tiles to the west of the enemy, facing east.
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        let cam = Camera {
            x: a.x - 2.0,
            y: a.y,
            angle: 0.0,
        };
        let mut zbuf = [f32::INFINITY; VIEW_W];
        render_walls(&mut fb, &data.vswap, &base.level, cam, &mut zbuf);
        let sprites = collect_sprites(&base.level, &base.actors, cam.angle);
        render_sprites(&mut fb, &data.vswap, cam, &zbuf, &sprites);
        draw_status_bar(&mut fb, &data.vga, &base.hud);
        let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
        for &idx in &fb.pixels {
            ppm.extend_from_slice(&palette::to_rgb(idx));
        }
        std::fs::write(std::env::temp_dir().join("wolf3d_enemy.ppm"), ppm).unwrap();
    }

    /// Render the `Escape` level-select overlay for visual inspection.
    #[test]
    fn render_level_select_menu() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        crate::render::hud::draw_level_select(&mut fb, &data.vga, 2, 4, 2);
        let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
        for &idx in &fb.pixels {
            ppm.extend_from_slice(&palette::to_rgb(idx));
        }
        std::fs::write(std::env::temp_dir().join("wolf3d_level_select.ppm"), ppm).unwrap();
    }

    #[test]
    fn render_text_overlay() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        fb.clear(0x00);
        crate::render::hud::draw_center_text(&mut fb, &data.vga, "LEVEL COMPLETE", 70, 4);
        crate::render::hud::draw_center_text(&mut fb, &data.vga, "GET PSYCHED!", 90, 2);
        let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
        for &idx in &fb.pixels {
            ppm.extend_from_slice(&palette::to_rgb(idx));
        }
        std::fs::write(std::env::temp_dir().join("wolf3d_text.ppm"), ppm).unwrap();
    }

    /// Render the on-screen touch gamepad over a game frame for inspection.
    #[test]
    fn render_touch_controls() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let world = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        let cam = Camera {
            x: world.player.x,
            y: world.player.y,
            angle: world.player.angle,
        };
        let mut zbuf = [f32::INFINITY; VIEW_W];
        render_walls(&mut fb, &data.vswap, &world.level, cam, &mut zbuf);
        draw_status_bar(&mut fb, &data.vga, &world.hud);

        let mut controls = crate::touch::TouchControls::default();
        controls.enabled = true;
        controls.movement = bevy::math::Vec2::new(0.0, 1.0);
        controls.fire = true;
        controls.run = true;
        controls.weapon = Some(2);
        crate::touch::draw_controls(&mut fb, &data.vga, &controls);

        let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
        for &idx in &fb.pixels {
            ppm.extend_from_slice(&palette::to_rgb(idx));
        }
        std::fs::write(std::env::temp_dir().join("wolf3d_touch.ppm"), ppm).unwrap();
    }

    /// Dump the "Get Psyched!" and high-score screens for visual inspection.
    #[test]
    fn render_menu_screens() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let dump = |fb: &Framebuffer, name: &str| {
            let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
            for &idx in &fb.pixels {
                ppm.extend_from_slice(&palette::to_rgb(idx));
            }
            std::fs::write(std::env::temp_dir().join(name), ppm).unwrap();
        };
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        crate::render::hud::draw_get_psyched(&mut fb, &data.vga);
        dump(&fb, "wolf3d_getpsyched.ppm");

        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        let mut scores = crate::game::scores::HighScores::default();
        scores.submit(12345, 0, 3);
        scores.submit(9999, 1, 7);
        scores.submit(500, 5, 0);
        crate::render::hud::draw_scores(&mut fb, &data.vga, &scores);
        dump(&fb, "wolf3d_scores.ppm");
    }

    /// Render the end-of-floor intermission for visual inspection.
    #[test]
    fn render_intermission() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        crate::render::hud::draw_intermission(
            &mut fb,
            &data.vga,
            &crate::render::hud::Intermission {
                secret_floor: false,
                time_secs: 83.0,
                par_secs: 90.0,
                kill: 100,
                secret: 50,
                treasure: 80,
                bonus: 12500,
            },
        );
        let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
        for &idx in &fb.pixels {
            ppm.extend_from_slice(&palette::to_rgb(idx));
        }
        std::fs::write(std::env::temp_dir().join("wolf3d_intermission.ppm"), ppm).unwrap();
    }

    /// Render the E1M1 pushwall at several points as it slides, for visual
    /// inspection.
    #[test]
    fn render_pushwall() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let mut world = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        world.player.x = 10.5;
        world.player.y = 14.5;
        world.player.angle = std::f32::consts::FRAC_PI_2; // north
        world.use_action();
        for (n, secs) in [0.4f32, 0.9, 1.5, 2.2, 3.0, 4.0].into_iter().enumerate() {
            // Re-simulate from the start each time.
            let mut world = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
            world.player.x = 10.5;
            world.player.y = 14.5;
            world.player.angle = std::f32::consts::FRAC_PI_2;
            world.use_action();
            for _ in 0..(secs * 60.0) as usize {
                world.update(1.0 / 60.0, &InputState::default());
            }
            let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
            let cam = Camera {
                x: world.player.x,
                y: world.player.y,
                angle: world.player.angle,
            };
            let mut zbuf = [f32::INFINITY; VIEW_W];
            render_walls(&mut fb, &data.vswap, &world.level, cam, &mut zbuf);
            let mut ppm = format!("P6\n{} {}\n255\n", VIEW_W, VIEW_H).into_bytes();
            for &idx in &fb.pixels {
                ppm.extend_from_slice(&palette::to_rgb(idx));
            }
            let out = std::env::temp_dir().join(format!("wolf3d_pushwall_{n}.ppm"));
            std::fs::write(&out, ppm).unwrap();
        }
        eprintln!("wrote pushwall frames");
    }

    /// Render every floor from four headings; catches sprite/texture indexing
    /// errors anywhere in the data set.
    #[test]
    fn render_every_map() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        for i in 0..60 {
            let world = World::new(&data, i / 10, i % 10, Difficulty::Hard).unwrap();
            for angle in [0.0f32, 1.5708, 3.1416, 4.7124] {
                let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
                let cam = Camera {
                    x: world.player.x,
                    y: world.player.y,
                    angle,
                };
                let mut zbuf = [f32::INFINITY; VIEW_W];
                render_walls(&mut fb, &data.vswap, &world.level, cam, &mut zbuf);
                let sprites = collect_sprites(&world.level, &world.actors, angle);
                render_sprites(&mut fb, &data.vswap, cam, &zbuf, &sprites);
                render_weapon(&mut fb, &data.vswap, world.player.weapon_sprite());
                draw_status_bar(&mut fb, &data.vga, &world.hud);
            }
        }
    }

    /// Regression: the camera sits `FOCAL` behind the player in the original,
    /// so a wall exactly one tile away is ~81.6 px tall (not `PROJ_V`).
    #[test]
    fn wall_height_matches_original_projection() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();

        let mut walls = vec![0u16; 64];
        for y in 0..8 {
            walls[y * 8 + 2] = 1; // solid wall spanning x = 2..3
        }
        let map = Map {
            width: 8,
            height: 8,
            name: "test".into(),
            walls,
            objects: vec![0; 64],
        };
        let level = build_level(&map, 0, 0);
        let cam = Camera {
            x: 1.0,
            y: 1.5,
            angle: 0.0, // facing east, one tile from the wall at x=2
        };
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        let mut zbuf = [f32::INFINITY; VIEW_W];
        render_walls(&mut fb, &data.vswap, &level, cam, &mut zbuf);

        let perp = zbuf[160];
        assert!(
            (perp - (1.0 + FOCAL)).abs() < 0.01,
            "expected perp 1+FOCAL, got {perp}"
        );
        let height = PROJ_V / perp;
        assert!((height - 81.6).abs() < 0.6, "unexpected wall height {height}");
    }
}
