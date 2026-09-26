//! A single-slot save game, written as a small line-based text file.
//!
//! SPDX-License-Identifier: MIT

use std::path::Path;

use crate::data::GameData;
use crate::game::actor::{ActorKind, ActorState, Difficulty};
use crate::game::level::{DoorState, PushDir};
use crate::game::world::World;

pub const FILE_NAME: &str = "wolf3d-bevy.sav";
const MAGIC: &str = "wolf3d-bevy save 1";

fn door_code(s: DoorState) -> u8 {
    match s {
        DoorState::Closed => 0,
        DoorState::Opening => 1,
        DoorState::Open => 2,
        DoorState::Closing => 3,
    }
}

fn door_from(c: u8) -> DoorState {
    match c {
        1 => DoorState::Opening,
        2 => DoorState::Open,
        3 => DoorState::Closing,
        _ => DoorState::Closed,
    }
}

fn actor_code(s: ActorState) -> u8 {
    match s {
        ActorState::Idle => 0,
        ActorState::Chase => 1,
        ActorState::Shoot => 2,
        ActorState::Bite => 3,
        ActorState::Pain => 4,
        ActorState::Dying => 5,
        ActorState::Dead => 6,
    }
}

fn actor_from(c: u8) -> ActorState {
    match c {
        1 => ActorState::Chase,
        2 => ActorState::Shoot,
        3 => ActorState::Bite,
        4 => ActorState::Pain,
        5 => ActorState::Dying,
        6 => ActorState::Dead,
        _ => ActorState::Idle,
    }
}

fn dir_code(d: PushDir) -> u8 {
    match d {
        PushDir::East => 0,
        PushDir::North => 1,
        PushDir::West => 2,
        PushDir::South => 3,
    }
}

/// Write `world` to `path`.
pub fn save(world: &World, path: &Path) -> std::io::Result<()> {
    let mut s = String::new();
    s.push_str(MAGIC);
    s.push('\n');
    let p = &world.player;
    s.push_str(&format!(
        "game {} {} {}\n",
        world.episode,
        world.map_index,
        world.difficulty.index()
    ));
    s.push_str(&format!(
        "player {:.5} {:.5} {:.5} {} {} {} {} {} {} {}\n",
        p.x, p.y, p.angle, p.health, p.ammo, p.score, p.lives, p.keys, p.weapon, p.best_weapon
    ));
    s.push_str(&format!("time {:.4}\n", world.elapsed));
    s.push_str(&format!(
        "counts {} {} {} {} {}\n",
        world.kill_total, world.kill_count, world.secret_count, world.treasure_count, world.last_bonus
    ));
    for d in &world.level.doors {
        s.push_str(&format!(
            "door {:.5} {} {:.4}\n",
            d.position,
            door_code(d.state),
            d.timer
        ));
    }
    if let Some(pw) = &world.level.push_wall {
        s.push_str(&format!(
            "push {} {} {} {} {:.5} {} {}\n",
            pw.active as u8,
            pw.x,
            pw.y,
            dir_code(pw.dir),
            pw.pos,
            pw.moved,
            pw.base
        ));
    }
    for a in &world.actors {
        s.push_str(&format!(
            "actor {} {:.5} {:.5} {} {} {} {:.5} {:.5} {:.5} {} {} {:.4} {:.4} {} {} {} {} {} {} {}\n",
            a.kind.index(),
            a.x,
            a.y,
            a.tile_x,
            a.tile_y,
            a.dir,
            a.distance,
            a.speed,
            a.facing,
            actor_code(a.state),
            a.health,
            a.timer,
            a.anim,
            a.walk_frame,
            a.patrol as u8,
            a.awake as u8,
            a.ambush as u8,
            a.first_attack as u8,
            a.area,
            a.shot_fired as u8
        ));
    }
    std::fs::write(path, s)
}

/// Rebuild a world from `path`, using `data` for the level and actors.
pub fn load(data: &GameData, path: &Path) -> Option<World> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    if lines.next()? != MAGIC {
        return None;
    }

    let mut episode = 0usize;
    let mut map = 0usize;
    let mut difficulty = Difficulty::Normal;
    let mut player_line: Option<Vec<f64>> = None;
    let mut time = 0.0f32;
    let mut counts = [0usize; 4];
    let mut bonus = 0i32;
    let mut doors: Vec<(f32, DoorState, f32)> = Vec::new();
    let mut push: Option<(bool, usize, usize, PushDir, f32, usize, u8)> = None;
    let mut actors: Vec<Vec<f64>> = Vec::new();

    for line in lines {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("game") => {
                episode = it.next()?.parse().ok()?;
                map = it.next()?.parse().ok()?;
                difficulty = Difficulty::from_index(it.next()?.parse().ok()?);
            }
            Some("player") => {
                player_line = Some(it.filter_map(|v| v.parse::<f64>().ok()).collect());
            }
            Some("time") => time = it.next()?.parse().ok()?,
            Some("counts") => {
                let vals: Vec<usize> = it.filter_map(|v| v.parse().ok()).collect();
                if vals.len() >= 5 {
                    counts.copy_from_slice(&vals[..4]);
                    bonus = vals[4] as i32;
                }
            }
            Some("door") => {
                let pos = it.next()?.parse().ok()?;
                let state = door_from(it.next()?.parse().ok()?);
                let timer = it.next()?.parse().ok()?;
                doors.push((pos, state, timer));
            }
            Some("push") => {
                let active = it.next()? == "1";
                let x = it.next()?.parse().ok()?;
                let y = it.next()?.parse().ok()?;
                let dir = PushDir::from_index(it.next()?.parse().ok()?);
                let pos = it.next()?.parse().ok()?;
                let moved = it.next()?.parse().ok()?;
                let base = it.next()?.parse().ok()?;
                push = Some((active, x, y, dir, pos, moved, base));
            }
            Some("actor") => {
                actors.push(it.filter_map(|v| v.parse::<f64>().ok()).collect());
            }
            _ => {}
        }
    }

    let mut world = World::new(data, episode, map, difficulty)?;

    if let Some(p) = player_line {
        if p.len() >= 10 {
            world.player.x = p[0] as f32;
            world.player.y = p[1] as f32;
            world.player.angle = p[2] as f32;
            world.player.health = p[3] as i32;
            world.player.ammo = p[4] as i32;
            world.player.score = p[5] as i32;
            world.player.lives = p[6] as i32;
            world.player.keys = p[7] as u8;
            world.player.weapon = p[8] as usize;
            world.player.best_weapon = p[9] as usize;
        }
    }
    world.elapsed = time;
    world.kill_total = counts[0];
    world.kill_count = counts[1];
    world.secret_count = counts[2];
    world.treasure_count = counts[3];
    world.last_bonus = bonus;

    for (i, (pos, state, timer)) in doors.into_iter().enumerate() {
        if let Some(d) = world.level.doors.get_mut(i) {
            d.position = pos;
            d.state = state;
            d.timer = timer;
        }
    }
    if let Some((active, x, y, dir, pos, moved, base)) = push {
        world.level.push_wall = Some(crate::game::level::PushWall {
            x,
            y,
            dir,
            pos,
            moved,
            base,
            active,
        });
    }
    for (i, v) in actors.into_iter().enumerate() {
        let Some(a) = world.actors.get_mut(i) else {
            continue;
        };
        if v.len() < 20 {
            continue;
        }
        a.kind = ActorKind::from_index(v[0] as usize);
        a.x = v[1] as f32;
        a.y = v[2] as f32;
        a.tile_x = v[3] as i32;
        a.tile_y = v[4] as i32;
        a.dir = v[5] as i32;
        a.distance = v[6] as f32;
        a.speed = v[7] as f32;
        a.facing = v[8] as f32;
        a.state = actor_from(v[9] as u8);
        a.health = v[10] as i32;
        a.timer = v[11] as f32;
        a.anim = v[12] as f32;
        a.walk_frame = v[13] as usize;
        a.patrol = v[14] != 0.0;
        a.awake = v[15] != 0.0;
        a.ambush = v[16] != 0.0;
        a.first_attack = v[17] != 0.0;
        a.area = v[18] as usize;
        a.shot_fired = v[19] != 0.0;
    }
    world.state = crate::game::world::PlayState::Playing;
    world.death_tint = 0.0;
    world.last_damage_source = None;
    world.sync_hud();
    Some(world)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_world() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = GameData::load(&dir).unwrap();
        let mut world = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        world.player.x += 0.25;
        world.player.health = 63;
        world.player.score = 4242;
        world.player.keys = 3;
        world.actors[0].awake = true;
        world.actors[0].health = 7;
        let path = std::env::temp_dir().join("wolf3d-bevy-roundtrip.sav");
        save(&world, &path).unwrap();
        let restored = load(&data, &path).unwrap();
        assert_eq!(restored.map_index, world.map_index);
        assert_eq!(restored.player.health, 63);
        assert_eq!(restored.player.score, 4242);
        assert_eq!(restored.player.keys, 3);
        assert!((restored.player.x - world.player.x).abs() < 0.01);
        assert_eq!(restored.actors.len(), world.actors.len());
        assert!(restored.actors[0].awake);
        assert_eq!(restored.actors[0].health, 7);
        let _ = std::fs::remove_file(&path);
    }
}
