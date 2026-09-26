//! Game rules: player movement, combat, doors, items and enemy AI.
//!
//! SPDX-License-Identifier: MIT

use crate::data::generated::*;
use crate::data::{GameData, Map};
use crate::game::actor::{Actor, ActorKind, ActorState, Difficulty, dir_angle, spawn_actors};
use crate::game::hud::Hud;
use crate::game::level::{DoorState, Item, Level, PushDir, PushWall, build_level};
use crate::game::level::{ELEVATOR_TILE, PUSHWALL_TILE};

/// Where completing a secret floor returns to, per episode (the original's
/// `ElevatorBackTo`).
pub const ELEVATOR_BACK_TO: [usize; 6] = [1, 1, 7, 3, 5, 3];

/// Sound chunk for a pushable wall sliding (the original's `PUSHWALLSND`).
const PUSHWALL_SND: usize = 46;
/// The player's score bonus for finishing a secret floor.
pub const SECRET_BONUS: i32 = 15_000;

/// Per-floor par times in minutes, from the original's `parTimes`. Zero marks
/// the boss and secret floors, which have no par time.
pub const PAR_TIMES: [f32; 60] = [
    1.5, 2.0, 2.0, 3.5, 3.0, 3.0, 2.5, 2.5, 0.0, 0.0, // Episode 1
    1.5, 3.5, 3.0, 2.0, 4.0, 6.0, 1.0, 3.0, 0.0, 0.0, // Episode 2
    1.5, 1.5, 2.5, 2.5, 3.5, 2.5, 2.0, 6.0, 0.0, 0.0, // Episode 3
    2.0, 2.0, 1.5, 1.0, 4.5, 3.5, 2.0, 4.5, 0.0, 0.0, // Episode 4
    2.5, 1.5, 2.5, 2.5, 4.0, 3.0, 4.5, 3.5, 0.0, 0.0, // Episode 5
    6.5, 4.0, 4.5, 6.0, 5.0, 5.5, 5.5, 8.5, 0.0, 0.0, // Episode 6
];

/// Radius of the player's collision circle, in tiles (`MINDIST`).
pub const PLAYER_RADIUS: f32 = 22528.0 / 65536.0;

pub const WALK_SPEED: f32 = 3.0;
pub const RUN_SPEED: f32 = 5.4;
pub const KEY_TURN_SPEED: f32 = 2.6;

const WEAPON_SPRITES: [u16; 4] = [
    SPR_KNIFEREADY,
    SPR_PISTOLREADY,
    SPR_MACHINEGUNREADY,
    SPR_CHAINREADY,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayState {
    Playing,
    Died,
    LevelComplete,
    GameOver,
}

#[derive(Clone, Default)]
pub struct InputState {
    /// -1 backward .. 1 forward.
    pub forward: f32,
    /// -1 left .. 1 right.
    pub strafe: f32,
    /// -1 .. 1 keyboard turning.
    pub turn: f32,
    /// Mouse movement in pixels.
    pub mouse_dx: f32,
    pub run: bool,
    pub fire: bool,
    /// Fire was just pressed this frame (for semi-auto weapons).
    pub fire_pressed: bool,
    pub use_pressed: bool,
    pub next_weapon: Option<usize>,
    pub prev_weapon: Option<usize>,
}

#[derive(Clone)]
pub struct Player {
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub health: i32,
    pub ammo: i32,
    pub score: i32,
    pub lives: i32,
    pub keys: u8,
    pub weapon: usize,
    pub best_weapon: usize,
    pub attack_timer: f32,
    pub attack_duration: f32,
    pub fire_cooldown: f32,
    pub face_frame: usize,
    pub face_timer: f32,
    pub moving: bool,
    pub damage_flash: f32,
    pub rng: u32,
}

impl Player {
    pub fn new(x: f32, y: f32, angle: f32) -> Self {
        Self {
            x,
            y,
            angle,
            health: 100,
            ammo: 8,
            score: 0,
            lives: 3,
            keys: 0,
            weapon: 1,
            best_weapon: 1,
            attack_timer: 0.0,
            attack_duration: 0.0,
            fire_cooldown: 0.0,
            face_frame: 1,
            face_timer: 0.0,
            moving: false,
            damage_flash: 0.0,
            rng: 0x1234_5678,
        }
    }

    pub fn rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// Weapon sprite to draw in the 3D view (ready or attack frame).
    pub fn weapon_sprite(&self) -> u16 {
        let base = WEAPON_SPRITES[self.weapon.min(3)];
        if self.attack_timer <= 0.0 || self.attack_duration <= 0.0 {
            return base;
        }
        let t = 1.0 - (self.attack_timer / self.attack_duration);
        let frame = (t * 4.0) as u16;
        base + frame.min(3)
    }
}

pub struct World {
    pub level: Level,
    pub actors: Vec<Actor>,
    pub player: Player,
    pub hud: Hud,
    pub episode: usize,
    pub map_index: usize,
    pub difficulty: Difficulty,
    pub state: PlayState,
    pub elapsed: f32,
    /// Sound chunk indices requested this frame.
    pub sounds: Vec<usize>,
    pub transition_timer: f32,
    /// Enemies spawned on this floor and killed so far.
    pub kill_total: usize,
    pub kill_count: usize,
    /// Pushwalls found (`secret_total` lives on the level).
    pub secret_count: usize,
    pub treasure_count: usize,
    /// The elevator used was the secret one; the next level is the secret floor.
    pub pending_secret: bool,
    /// Score awarded by the intermission for the floor just finished.
    pub last_bonus: i32,
    /// Par time of the current floor in minutes (0 for boss/secret floors).
    pub par_time: f32,
}

impl World {
    pub fn new(
        data: &GameData,
        episode: usize,
        map_index: usize,
        difficulty: Difficulty,
    ) -> Option<Self> {
        let map = data.maps.get(episode * 10 + map_index)?;
        Some(Self::from_map(map, episode, map_index, difficulty))
    }

    pub fn from_map(map: &Map, episode: usize, map_index: usize, difficulty: Difficulty) -> Self {
        let level = build_level(map, episode, map_index);
        let actors = spawn_actors(level.width, level.height, &map.objects, difficulty);
        let (px, py, pa) = level.player_start;
        let player = Player::new(px, py, pa);
        let hud = Hud {
            health: player.health,
            ammo: player.ammo,
            score: player.score,
            lives: player.lives,
            keys: player.keys,
            weapon: player.weapon,
            face_frame: player.face_frame,
            level: map_index + 1,
        };
        let kill_total = actors.len();
        Self {
            level,
            actors,
            player,
            hud,
            episode,
            map_index,
            difficulty,
            state: PlayState::Playing,
            elapsed: 0.0,
            sounds: Vec::new(),
            transition_timer: 0.0,
            kill_total,
            kill_count: 0,
            secret_count: 0,
            treasure_count: 0,
            pending_secret: false,
            last_bonus: 0,
            par_time: PAR_TIMES[(episode * 10 + map_index).min(PAR_TIMES.len() - 1)],
        }
    }

    pub fn kill_percent(&self) -> i32 {
        if self.kill_total == 0 {
            0
        } else {
            (self.kill_count * 100 / self.kill_total) as i32
        }
    }

    pub fn secret_percent(&self) -> i32 {
        if self.level.secret_total == 0 {
            0
        } else {
            (self.secret_count * 100 / self.level.secret_total) as i32
        }
    }

    pub fn treasure_percent(&self) -> i32 {
        if self.level.treasure_total == 0 {
            0
        } else {
            (self.treasure_count * 100 / self.level.treasure_total) as i32
        }
    }

    /// Advance the world by `dt` seconds.
    pub fn update(&mut self, dt: f32, input: &InputState) {
        self.sounds.clear();
        if self.state != PlayState::Playing {
            self.transition_timer += dt;
            return;
        }
        self.elapsed += dt;
        self.player.damage_flash = (self.player.damage_flash - dt).max(0.0);

        self.update_player(dt, input);
        self.update_doors(dt);
        self.update_push_wall(dt);
        self.update_actors(dt);
        self.update_items();
        self.check_exit();
        self.sync_hud();
    }

    fn update_player(&mut self, dt: f32, input: &InputState) {
        // Turning.
        let mouse_turn = input.mouse_dx * 0.003;
        self.player.angle -= input.turn * KEY_TURN_SPEED * dt + mouse_turn;
        self.player.angle = self.player.angle.rem_euclid(std::f32::consts::TAU);

        // Movement.
        let (dx, dy) = (self.player.angle.cos(), -self.player.angle.sin());
        // Strafe direction is the camera plane direction.
        let (sx, sy) = (-dy, dx);
        let speed = if input.run { RUN_SPEED } else { WALK_SPEED };
        let mut mx = dx * input.forward + sx * input.strafe;
        let mut my = dy * input.forward + sy * input.strafe;
        let len = (mx * mx + my * my).sqrt();
        if len > 0.0001 {
            mx = mx / len * speed * dt;
            my = my / len * speed * dt;
            self.player.moving = true;
        } else {
            self.player.moving = false;
        }
        self.move_player(mx, my);

        // Weapons.
        self.player.fire_cooldown = (self.player.fire_cooldown - dt).max(0.0);
        if let Some(w) = input.next_weapon {
            self.switch_weapon(w);
        }
        if let Some(w) = input.prev_weapon {
            self.switch_weapon(w);
        }
        self.player.attack_timer = (self.player.attack_timer - dt).max(0.0);
        let auto = self.player.weapon >= 2;
        let wants_fire = input.fire && (auto || input.fire_pressed);
        if wants_fire && self.player.fire_cooldown <= 0.0 && self.player.attack_timer <= 0.0 {
            self.fire();
        }

        // Face animation.
        self.player.face_timer += dt;
        if self.player.face_timer > 1.5 {
            self.player.face_timer = 0.0;
            let r = self.player.rand() % 3;
            self.player.face_frame = if r == 3 { 1 } else { r as usize };
        }
    }

    fn switch_weapon(&mut self, weapon: usize) {
        if weapon <= self.player.best_weapon && weapon != self.player.weapon {
            self.player.weapon = weapon;
            self.player.attack_timer = 0.0;
            self.sounds.push(1); // SELECTWPNSND
        }
    }

    fn move_player(&mut self, dx: f32, dy: f32) {
        let r = PLAYER_RADIUS;
        let nx = self.player.x + dx;
        if !collides(&self.level, nx, self.player.y, r) {
            self.player.x = nx;
        }
        let ny = self.player.y + dy;
        if !collides(&self.level, self.player.x, ny, r) {
            self.player.y = ny;
        }
    }

    fn fire(&mut self) {
        let weapon = self.player.weapon;
        let (duration, interval, ammo, sound) = match weapon {
            0 => (0.35, 0.4, 0, 23), // knife
            1 => (0.28, 0.35, 1, 24), // pistol
            2 => (0.25, 0.14, 1, 26), // machine gun
            _ => (0.25, 0.09, 1, 11), // gatling
        };
        if ammo > 0 {
            if self.player.ammo <= 0 {
                self.sounds.push(13); // NOITEMSND
                return;
            }
            self.player.ammo -= ammo;
        }
        self.player.attack_timer = duration;
        self.player.attack_duration = duration;
        self.player.fire_cooldown = interval;
        self.sounds.push(sound);
        self.hitscan(weapon);
        self.sync_hud();
    }

    /// Player hitscan: pick the closest visible actor near the aim centre.
    fn hitscan(&mut self, weapon: usize) {
        let (dx, dy) = (self.player.angle.cos(), -self.player.angle.sin());
        let range = if weapon == 0 { 1.6 } else { 22.0 };
        let mut best: Option<(usize, f32)> = None;
        for (i, a) in self.actors.iter().enumerate() {
            if !a.is_alive() {
                continue;
            }
            let rx = a.x - self.player.x;
            let ry = a.y - self.player.y;
            let dist = (rx * rx + ry * ry).sqrt();
            if dist > range {
                continue;
            }
            // Project onto the view axis.
            let depth = rx * dx + ry * dy;
            if depth <= 0.0 {
                continue;
            }
            let lateral = (-rx * dy + ry * dx).abs();
            // Allow a generous cone (the original uses viewwidth/10 pixels).
            let allowed = 0.18 + depth * 0.18;
            if lateral > allowed {
                continue;
            }
            if !line_of_sight(&self.level, self.player.x, self.player.y, a.x, a.y) {
                continue;
            }
            if best.map(|(_, d)| dist < d).unwrap_or(true) {
                best = Some((i, dist));
            }
        }
        if let Some((i, dist)) = best {
            let mut damage = (self.player.rand() % 16) as i32;
            if weapon == 0 {
                damage = 15 + (self.player.rand() % 10) as i32;
            } else if dist > 4.0 && (self.player.rand() % 12) as f32 / 12.0 * 12.0 < dist {
                // Long-range miss chance.
                return;
            }
            self.damage_actor(i, damage);
            self.sounds.push(27); // HITENEMYSND
        } else {
            self.sounds.push(0); // HITWALLSND
        }
    }

    fn damage_actor(&mut self, index: usize, damage: i32) {
        let Some(a) = self.actors.get_mut(index) else {
            return;
        };
        if !a.is_alive() {
            return;
        }
        a.health -= damage;
        if a.health <= 0 {
            a.state = ActorState::Dying;
            a.timer = 0.0;
            self.player.score += 100;
            self.kill_count += 1;
            self.sounds.push(death_sound(a.kind));
        } else {
            a.awake = true;
            if a.kind.pain_sprite() != 0 {
                a.state = ActorState::Pain;
                a.timer = 0.25;
            }
        }
    }

    fn update_doors(&mut self, dt: f32) {
        // A closing door must not trap the player or an actor. The original
        // checks this in `CloseDoor` and re-checks in `DoorClosing`, reopening
        // whenever something is in the doorway.
        let occupied: Vec<bool> = self
            .level
            .doors
            .iter()
            .map(|d| self.door_occupied(d.x, d.y))
            .collect();
        for (i, door) in self.level.doors.iter_mut().enumerate() {
            match door.state {
                DoorState::Opening => {
                    door.position += dt / 0.9;
                    if door.position >= 1.0 {
                        door.position = 1.0;
                        door.state = DoorState::Open;
                        door.timer = 4.3;
                    }
                }
                DoorState::Open => {
                    door.timer -= dt;
                    if door.timer <= 0.0 && !occupied[i] {
                        door.state = DoorState::Closing;
                    }
                }
                DoorState::Closing => {
                    if occupied[i] {
                        // Something moved into the doorway; reopen instead of
                        // crushing it.
                        door.state = DoorState::Opening;
                    } else {
                        door.position -= dt / 0.9;
                        if door.position <= 0.0 {
                            door.position = 0.0;
                            door.state = DoorState::Closed;
                        }
                    }
                }
                DoorState::Closed => {}
            }
        }
    }

    /// Whether the player or a live actor overlaps the given door tile.
    fn door_occupied(&self, x: usize, y: usize) -> bool {
        if circle_overlaps_tile(self.player.x, self.player.y, PLAYER_RADIUS, x, y) {
            return true;
        }
        self.actors
            .iter()
            .any(|a| a.is_alive() && circle_overlaps_tile(a.x, a.y, PLAYER_RADIUS, x, y))
    }

    /// Slide the active pushwall and open the passage behind it.
    fn update_push_wall(&mut self, dt: f32) {
        let Some(mut pw) = self.level.push_wall.take() else {
            return;
        };
        if pw.active {
            // The original crosses one tile per 128 tics (~1.8 s), so about
            // 0.55 tiles/s; two tiles take roughly 3.6 s.
            pw.pos += 0.55 * dt;
            while pw.pos >= 1.0 {
                pw.pos -= 1.0;
                // The tile the wall leaves becomes floor.
                self.level.tilemap[pw.y * self.level.width + pw.x] = 0;
                let (dx, dy) = pw.dir.delta();
                let nx = pw.x as i32 + dx;
                let ny = pw.y as i32 + dy;
                if nx < 0
                    || ny < 0
                    || nx as usize >= self.level.width
                    || ny as usize >= self.level.height
                {
                    pw.active = false;
                    break;
                }
                pw.x = nx as usize;
                pw.y = ny as usize;
                pw.moved += 1;
                let idx = pw.y * self.level.width + pw.x;
                if pw.moved >= 2 {
                    // The wall comes to rest and stays solid.
                    self.level.tilemap[idx] = pw.base;
                    pw.active = false;
                    break;
                }
                self.level.tilemap[idx] = 0xC0 | pw.base;
                // Pre-mark the next tile solid, as the original does; if it is
                // blocked the wall stops here.
                let nnx = pw.x as i32 + dx;
                let nny = pw.y as i32 + dy;
                if nnx < 0
                    || nny < 0
                    || nnx as usize >= self.level.width
                    || nny as usize >= self.level.height
                    || self.level.is_solid(nnx as usize, nny as usize)
                {
                    self.level.tilemap[idx] = pw.base;
                    pw.active = false;
                    break;
                }
                self.level.tilemap[nny as usize * self.level.width + nnx as usize] = pw.base;
            }
        }
        self.level.push_wall = Some(pw);
    }

    fn update_actors(&mut self, dt: f32) {
        let px = self.player.x;
        let py = self.player.y;
        let player_alive = self.player.health > 0;
        let mut damage_to_player = 0i32;

        for i in 0..self.actors.len() {
            let (ax, ay, state) = {
                let a = &self.actors[i];
                (a.x, a.y, a.state)
            };
            match state {
                ActorState::Dead => continue,
                ActorState::Dying => {
                    let a = &mut self.actors[i];
                    a.timer += dt;
                    if a.timer > 0.18 * a.kind.die_frames() as f32 {
                        a.state = ActorState::Dead;
                    }
                    continue;
                }
                ActorState::Pain => {
                    let a = &mut self.actors[i];
                    a.timer -= dt;
                    if a.timer <= 0.0 {
                        a.state = ActorState::Chase;
                    }
                    continue;
                }
                ActorState::Shoot => {
                    let a = &mut self.actors[i];
                    a.timer += dt;
                    if !a.shot_fired && a.timer > 0.18 {
                        a.shot_fired = true;
                        if player_alive
                            && line_of_sight(&self.level, a.x, a.y, px, py)
                            && dist(a.x, a.y, px, py) < a.kind.attack_range() + 2.0
                        {
                            damage_to_player += a.kind.damage();
                        }
                    }
                    if a.timer > 0.45 {
                        a.state = ActorState::Chase;
                        a.shot_fired = false;
                    }
                    continue;
                }
                ActorState::Idle | ActorState::Chase => {}
            }

            // Wake on sight.
            if !self.actors[i].awake {
                let d = dist(ax, ay, px, py);
                if d < 16.0 && line_of_sight(&self.level, ax, ay, px, py) {
                    self.actors[i].awake = true;
                }
            }

            if !self.actors[i].awake {
                if self.actors[i].patrol {
                    self.patrol(i, dt);
                }
                continue;
            }

            let a = &mut self.actors[i];
            let target = (px - a.x).atan2(-(py - a.y));
            // Turn toward the player smoothly.
            let mut diff = (target - a.facing + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let max_turn = 4.0 * dt;
            diff = diff.clamp(-max_turn, max_turn);
            a.facing = (a.facing + diff).rem_euclid(std::f32::consts::TAU);
            a.state = ActorState::Chase;
            a.anim += dt;
            if a.anim > 0.18 {
                a.anim = 0.0;
                a.walk_frame = (a.walk_frame + 1) % a.kind.walk_frames();
            }

            let d = dist(a.x, a.y, px, py);
            let los = line_of_sight(&self.level, a.x, a.y, px, py);
            if los && d < a.kind.attack_range() {
                a.state = ActorState::Shoot;
                a.timer = 0.0;
                a.shot_fired = false;
                self.sounds.push(attack_sound(a.kind));
                continue;
            }

            // Move toward the player.
            let speed = a.kind.speed();
            let (mvx, mvy) = if d > 0.0001 {
                let step = speed * dt;
                ((px - a.x) / d * step, (py - a.y) / d * step)
            } else {
                (0.0, 0.0)
            };
            if !self.move_actor(i, mvx, mvy) {
                // Blocked: try to sidestep around whatever is in the way.
                if !self.try_move_actor(i, mvy, -mvx) {
                    self.try_move_actor(i, -mvy, mvx);
                }
            }
        }

        // Sight and sound alerts spread to nearby actors, so a room wakes up
        // together instead of one guard at a time.
        let alerters: Vec<(f32, f32)> = self
            .actors
            .iter()
            .filter(|a| a.awake && a.is_alive())
            .map(|a| (a.x, a.y))
            .collect();
        if !alerters.is_empty() {
            for a in self.actors.iter_mut() {
                if a.awake || !a.is_alive() {
                    continue;
                }
                if alerters
                    .iter()
                    .any(|&(x, y)| dist(x, y, a.x, a.y) < 10.0)
                {
                    a.awake = true;
                }
            }
        }

        if damage_to_player > 0 && player_alive {
            self.player.health -= damage_to_player;
            self.player.damage_flash = 0.25;
            self.sounds.push(16); // TAKEDAMAGESND
            if self.player.health <= 0 {
                self.player.health = 0;
                self.state = PlayState::Died;
                self.transition_timer = 0.0;
                self.sounds.push(9); // PLAYERDEATHSND
            }
        }
    }

    fn patrol(&mut self, i: usize, dt: f32) {
        let a = &self.actors[i];
        let (dx, dy) = (a.facing.cos(), -a.facing.sin());
        let step = 0.6 * dt;
        let moved = self.try_move_actor(i, dx * step, dy * step);
        if !moved {
            // Bounce off walls.
            self.actors[i].facing = (self.actors[i].facing + std::f32::consts::PI / 2.0)
                .rem_euclid(std::f32::consts::TAU);
        }
        let a = &mut self.actors[i];
        a.anim += dt;
        if a.anim > 0.25 {
            a.anim = 0.0;
            a.walk_frame = (a.walk_frame + 1) % a.kind.walk_frames();
        }
    }

    /// Move a chasing actor, opening a closed door in the way like the
    /// original's `TryWalk`.
    fn move_actor(&mut self, i: usize, dx: f32, dy: f32) -> bool {
        let (ax, ay) = (self.actors[i].x, self.actors[i].y);
        let (tx, ty) = if dx.abs() > dy.abs() {
            ((ax + dx.signum() * 0.6).floor() as i32, ay.floor() as i32)
        } else {
            (ax.floor() as i32, (ay + dy.signum() * 0.6).floor() as i32)
        };
        if tx >= 0
            && ty >= 0
            && (tx as usize) < self.level.width
            && (ty as usize) < self.level.height
        {
            if let Some(d) = self.level.is_door(tx as usize, ty as usize) {
                if self.level.doors[d].state == DoorState::Closed && self.level.doors[d].lock == 0 {
                    self.level.doors[d].state = DoorState::Opening;
                    self.sounds.push(18); // OPENDOORSND
                }
            }
        }
        self.try_move_actor(i, dx, dy)
    }

    fn try_move_actor(&mut self, i: usize, dx: f32, dy: f32) -> bool {
        let (x, y) = (self.actors[i].x, self.actors[i].y);
        let r = 0.3;
        let mut moved = false;
        if dx != 0.0 && !collides(&self.level, x + dx, y, r) {
            self.actors[i].x = x + dx;
            moved = true;
        }
        let x = self.actors[i].x;
        if dy != 0.0 && !collides(&self.level, x, y + dy, r) {
            self.actors[i].y = y + dy;
            moved = true;
        }
        moved
    }

    fn update_items(&mut self) {
        let px = self.player.x;
        let py = self.player.y;
        for i in 0..self.level.statics.len() {
            if !self.level.statics[i].active {
                continue;
            }
            let sx = self.level.statics[i].x as f32 + 0.5;
            let sy = self.level.statics[i].y as f32 + 0.5;
            if dist(px, py, sx, sy) < 0.55 {
                self.pickup(i);
            }
        }
    }

    fn pickup(&mut self, i: usize) {
        let item = self.level.statics[i].item;
        let mut consumed = true;
        match item {
            Item::Dressing | Item::Block => consumed = false,
            Item::FirstAid => {
                if self.player.health >= 100 {
                    consumed = false;
                } else {
                    self.player.health = (self.player.health + 25).min(100);
                    self.sounds.push(34);
                }
            }
            Item::Food => {
                if self.player.health >= 100 {
                    consumed = false;
                } else {
                    self.player.health = (self.player.health + 10).min(100);
                    self.sounds.push(33);
                }
            }
            Item::Alpo => {
                if self.player.health >= 100 {
                    consumed = false;
                } else {
                    self.player.health = (self.player.health + 4).min(100);
                    self.sounds.push(33);
                }
            }
            Item::Gibs => {
                if self.player.health > 10 {
                    consumed = false;
                } else {
                    self.player.health = (self.player.health + 1).min(100);
                    self.sounds.push(33);
                }
            }
            Item::FullHeal => {
                self.player.health = 100;
                self.player.ammo = (self.player.ammo + 25).min(99);
                self.player.lives = (self.player.lives + 1).min(9);
                self.sounds.push(44);
            }
            Item::Key1 => {
                self.player.keys |= 1;
                self.sounds.push(12);
            }
            Item::Key2 => {
                self.player.keys |= 2;
                self.sounds.push(12);
            }
            Item::Key3 => {
                self.player.keys |= 4;
                self.sounds.push(12);
            }
            Item::Key4 => {
                self.player.keys |= 8;
                self.sounds.push(12);
            }
            Item::Cross => self.give_points(100, 35),
            Item::Chalice => self.give_points(500, 36),
            Item::Bible => self.give_points(1000, 37),
            Item::Crown => self.give_points(5000, 45),
            Item::Clip => {
                if self.player.ammo >= 99 {
                    consumed = false;
                } else {
                    self.player.ammo = (self.player.ammo + 8).min(99);
                    self.sounds.push(31);
                }
            }
            Item::Clip2 => {
                if self.player.ammo >= 99 {
                    consumed = false;
                } else {
                    self.player.ammo = (self.player.ammo + 4).min(99);
                    self.sounds.push(31);
                }
            }
            Item::Clip25 => {
                if self.player.ammo >= 99 {
                    consumed = false;
                } else {
                    self.player.ammo = (self.player.ammo + 25).min(99);
                    self.sounds.push(31);
                }
            }
            Item::MachineGun => {
                self.give_weapon(2);
                self.sounds.push(30);
            }
            Item::ChainGun => {
                self.give_weapon(3);
                self.sounds.push(38);
            }
            Item::Spear => consumed = false,
        }
        if consumed {
            self.level.statics[i].active = false;
            if item.is_treasure() {
                self.treasure_count += 1;
            }
        }
    }

    fn give_points(&mut self, points: i32, sound: usize) {
        self.player.score += points;
        self.sounds.push(sound);
    }

    fn give_weapon(&mut self, weapon: usize) {
        self.player.ammo = (self.player.ammo + 6).min(99);
        if weapon > self.player.best_weapon {
            self.player.best_weapon = weapon;
            self.player.weapon = weapon;
        }
    }

    fn check_exit(&mut self) {
        let tx = self.player.x.floor() as usize;
        let ty = self.player.y.floor() as usize;
        if tx < self.level.width && ty < self.level.height
            && self.level.object(tx, ty) == crate::game::level::EXIT_TILE
        {
            // The exit tile only opens once the floor's boss is dead.
            let boss_alive = self
                .actors
                .iter()
                .any(|a| a.kind.is_boss() && a.is_alive());
            if !boss_alive {
                self.complete_level();
            }
        }
    }

    /// Handle the "use" action (doors, elevator switches, pushwalls).
    pub fn use_action(&mut self) {
        if self.state != PlayState::Playing {
            return;
        }
        // Face one of the four cardinal directions, like Cmd_Use.
        let (dx, dy) = (self.player.angle.cos(), -self.player.angle.sin());
        let (mut ux, mut uy, east_west) = if dx.abs() > dy.abs() {
            let step = if dx > 0.0 { 1i32 } else { -1 };
            (
                (self.player.x.floor() as i32 + step).max(0) as usize,
                self.player.y.floor() as usize,
                true,
            )
        } else {
            let step = if dy > 0.0 { 1i32 } else { -1 };
            (
                self.player.x.floor() as usize,
                (self.player.y.floor() as i32 + step).max(0) as usize,
                false,
            )
        };
        if ux >= self.level.width {
            ux = self.level.width - 1;
        }
        if uy >= self.level.height {
            uy = self.level.height - 1;
        }
        let push_dir = if east_west {
            if dx > 0.0 { PushDir::East } else { PushDir::West }
        } else if dy > 0.0 {
            PushDir::South
        } else {
            PushDir::North
        };

        // Pushable wall. The original checks plane 1 for the marker first.
        if self.level.object(ux, uy) == PUSHWALL_TILE {
            self.push_wall(ux, uy, push_dir);
            return;
        }

        // Elevator switch. Compare the raw tilemap byte like the original's
        // `doornum == ELEVATORTILE`: masking off the high bits would make door
        // number 21 (`0x80 | 21`) look like the switch and end the level.
        if east_west && self.level.tile(ux, uy) == ELEVATOR_TILE as u8 {
            let (ptx, pty) = (
                self.player.x.floor() as usize,
                self.player.y.floor() as usize,
            );
            self.pending_secret = self.level.is_alt_elevator(ptx, pty);
            self.sounds.push(40); // LEVELDONESND
            self.complete_level();
            return;
        }

        // Doors.
        if let Some(door_index) = self.level.is_door(ux, uy) {
            self.operate_door(door_index);
            return;
        }
        self.sounds.push(20); // DONOTHINGSND
    }

    /// Begin pushing the wall at `(x, y)` in `dir`, if it can move.
    fn push_wall(&mut self, x: usize, y: usize, dir: PushDir) {
        if self
            .level
            .push_wall
            .as_ref()
            .map(|p| p.active)
            .unwrap_or(false)
        {
            self.sounds.push(6); // NOWAYSND
            return;
        }
        let base = self.level.tile(x, y) & 0x3F;
        if base == 0 {
            self.sounds.push(20); // DONOTHINGSND
            return;
        }
        let (dx, dy) = dir.delta();
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0
            || ny < 0
            || nx as usize >= self.level.width
            || ny as usize >= self.level.height
            || self.level.is_solid(nx as usize, ny as usize)
        {
            self.sounds.push(6); // NOWAYSND
            return;
        }
        let (nxu, nyu) = (nx as usize, ny as usize);
        self.level.tilemap[y * self.level.width + x] = 0xC0 | base;
        self.level.tilemap[nyu * self.level.width + nxu] = base;
        self.level.push_wall = Some(PushWall {
            x,
            y,
            dir,
            pos: 0.0,
            moved: 0,
            base,
            active: true,
        });
        self.secret_count += 1;
        self.sounds.push(PUSHWALL_SND);
        self.sync_hud();
    }

    fn operate_door(&mut self, index: usize) {
        let Some(door) = self.level.doors.get(index) else {
            return;
        };
        if door.lock >= 1 && door.lock <= 4 {
            let bit = 1u8 << (door.lock - 1);
            if self.player.keys & bit == 0 {
                self.sounds.push(6); // NOWAYSND
                return;
            }
        }
        match door.state {
            DoorState::Closed | DoorState::Closing => {
                self.level.doors[index].state = DoorState::Opening;
                self.sounds.push(18); // OPENDOORSND
            }
            DoorState::Open | DoorState::Opening => {
                self.level.doors[index].state = DoorState::Closing;
                self.sounds.push(19); // CLOSEDOORSND
            }
        }
    }

    fn complete_level(&mut self) {
        if self.state == PlayState::LevelComplete {
            return;
        }
        self.state = PlayState::LevelComplete;
        self.transition_timer = 0.0;
        self.last_bonus = self.compute_bonus();
        self.player.score += self.last_bonus;
        self.sync_hud();
    }

    /// End-of-floor bonus, mirroring `LevelCompleted`: a time bonus against par
    /// plus `PERCENT100AMT` for each 100% ratio.
    fn compute_bonus(&self) -> i32 {
        const PERCENT100AMT: i32 = 10_000;
        let mut bonus = 0;
        if self.par_time > 0.0 {
            let left = (self.par_time * 60.0 - self.elapsed).max(0.0);
            bonus += (left * 500.0) as i32;
        }
        if self.kill_total > 0 && self.kill_percent() == 100 {
            bonus += PERCENT100AMT;
        }
        if self.level.secret_total > 0 && self.secret_percent() == 100 {
            bonus += PERCENT100AMT;
        }
        if self.level.treasure_total > 0 && self.treasure_percent() == 100 {
            bonus += PERCENT100AMT;
        }
        bonus
    }

    /// Advance to the next map, keeping the player's stats.
    ///
    /// Mirrors the original's progression: floors 1..=8 go to the next floor,
    /// the boss floor (index 8) advances the episode, the secret floor (index
    /// 9) returns to `ElevatorBackTo`, and a secret elevator jumps to index 9.
    pub fn next_level(&mut self, data: &GameData) {
        let (next, episode) = if self.pending_secret {
            (9, self.episode)
        } else if self.map_index == 9 {
            (ELEVATOR_BACK_TO[self.episode.min(5)], self.episode)
        } else if self.map_index >= 8 {
            (0, (self.episode + 1) % 6)
        } else {
            (self.map_index + 1, self.episode)
        };
        self.pending_secret = false;
        if self.map_index == 9 {
            // The original awards a fixed bonus for finishing a secret floor.
            self.player.score += SECRET_BONUS;
        }
        self.load_map(data, episode, next);
    }

    /// Resolve a death once the pause has elapsed: spend a life and restart,
    /// or end the game when none are left.
    pub fn after_death(&mut self, data: &GameData) -> bool {
        if self.player.lives > 0 {
            self.player.lives -= 1;
            self.restart_level(data);
            false
        } else {
            self.state = PlayState::GameOver;
            self.transition_timer = 0.0;
            self.sounds.push(17); // GAMEOVERSND
            true
        }
    }

    /// Restart the current level after death. The caller deducts a life.
    /// Like the original's `Died`, the player loses their keys and weapons.
    pub fn restart_level(&mut self, data: &GameData) {
        self.player.health = 100;
        self.player.ammo = 8;
        self.player.weapon = 1;
        self.player.best_weapon = 1;
        self.player.keys = 0;
        self.pending_secret = false;
        let (episode, map) = (self.episode, self.map_index);
        self.load_map(data, episode, map);
    }

    /// Replace the current map, resetting per-floor counters.
    fn load_map(&mut self, data: &GameData, episode: usize, map_index: usize) {
        let Some(map) = data.maps.get(episode * 10 + map_index) else {
            return;
        };
        let level = build_level(map, episode, map_index);
        let actors = spawn_actors(level.width, level.height, &map.objects, self.difficulty);
        let (px, py, pa) = level.player_start;
        self.kill_total = actors.len();
        self.kill_count = 0;
        self.secret_count = 0;
        self.treasure_count = 0;
        self.level = level;
        self.actors = actors;
        self.player.x = px;
        self.player.y = py;
        self.player.angle = pa;
        self.player.attack_timer = 0.0;
        self.player.damage_flash = 0.0;
        self.map_index = map_index;
        self.episode = episode;
        self.state = PlayState::Playing;
        self.transition_timer = 0.0;
        self.elapsed = 0.0;
        self.last_bonus = 0;
        self.par_time = PAR_TIMES[(episode * 10 + map_index).min(PAR_TIMES.len() - 1)];
        self.sync_hud();
    }

    fn sync_hud(&mut self) {
        self.hud.health = self.player.health;
        self.hud.ammo = self.player.ammo;
        self.hud.score = self.player.score;
        self.hud.lives = self.player.lives;
        self.hud.keys = self.player.keys;
        self.hud.weapon = self.player.weapon;
        self.hud.face_frame = self.player.face_frame;
        self.hud.level = self.map_index + 1;
    }
}

fn dist(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt()
}

fn attack_sound(kind: ActorKind) -> usize {
    match kind {
        ActorKind::Dog => 41, // DOGBARKSND
        _ => 7,               // NAZIHITPLAYERSND
    }
}

fn death_sound(kind: ActorKind) -> usize {
    match kind {
        ActorKind::Dog => 10,
        ActorKind::Guard | ActorKind::Officer | ActorKind::Ss | ActorKind::Mutant => {
            22 + (kind as usize % 2) * 3
        }
        _ => 29,
    }
}

/// Whether a circle centred at `(x, y)` with radius `r` overlaps tile
/// `(tx, ty)`.
fn circle_overlaps_tile(x: f32, y: f32, r: f32, tx: usize, ty: usize) -> bool {
    let cx = x.clamp(tx as f32, tx as f32 + 1.0);
    let cy = y.clamp(ty as f32, ty as f32 + 1.0);
    (x - cx).powi(2) + (y - cy).powi(2) < r * r
}

/// Circle-vs-grid collision test.
pub fn collides(level: &Level, x: f32, y: f32, r: f32) -> bool {
    let min_x = (x - r).floor() as i32;
    let max_x = (x + r).floor() as i32;
    let min_y = (y - r).floor() as i32;
    let max_y = (y + r).floor() as i32;
    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            if tx < 0 || ty < 0 {
                return true;
            }
            let (txu, tyu) = (tx as usize, ty as usize);
            if txu >= level.width || tyu >= level.height {
                return true;
            }
            if !level.is_solid(txu, tyu) {
                continue;
            }
            if circle_overlaps_tile(x, y, r, txu, tyu) {
                return true;
            }
        }
    }
    false
}

/// Simple sampled line-of-sight test. Open doors do not block.
pub fn line_of_sight(level: &Level, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let dist = (dx * dx + dy * dy).sqrt();
    let steps = (dist * 8.0).ceil() as i32;
    if steps <= 0 {
        return true;
    }
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let x = x0 + dx * t;
        let y = y0 + dy * t;
        let tx = x.floor() as i32;
        let ty = y.floor() as i32;
        if tx < 0 || ty < 0 || tx as usize >= level.width || ty as usize >= level.height {
            return false;
        }
        let tile = level.tile(tx as usize, ty as usize);
        if tile & 0x80 != 0 {
            let d = (tile & 0x7F) as usize;
            if level.doors.get(d).map(|d| d.position < 0.9).unwrap_or(true) {
                return false;
            }
        } else if tile != 0 {
            return false;
        }
    }
    true
}

/// A helper for spawn directions.
pub fn facing_for_dir(dir: u16) -> f32 {
    dir_angle(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::game::actor::Difficulty;

    fn data() -> Option<GameData> {
        let dir = crate::data::find_data_dir()?;
        Some(GameData::load(dir).ok()?)
    }

    fn input_forward(v: f32) -> InputState {
        InputState {
            forward: v,
            ..Default::default()
        }
    }

    #[test]
    fn walking_never_enters_a_wall() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        for _ in 0..400 {
            w.update(1.0 / 60.0, &input_forward(1.0));
            assert!(
                !collides(&w.level, w.player.x, w.player.y, PLAYER_RADIUS),
                "player ended up inside a wall at {},{}",
                w.player.x,
                w.player.y
            );
        }
    }

    #[test]
    fn doors_do_not_close_on_the_player() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        // Stand in the first door tile with the door open and its timer expired.
        let (dx, dy) = (w.level.doors[0].x, w.level.doors[0].y);
        w.player.x = dx as f32 + 0.5;
        w.player.y = dy as f32 + 0.5;
        w.level.doors[0].position = 1.0;
        w.level.doors[0].state = DoorState::Open;
        w.level.doors[0].timer = 0.0;
        w.update(1.0 / 60.0, &InputState::default());
        assert_eq!(
            w.level.doors[0].state,
            DoorState::Open,
            "an open door started closing on the player"
        );

        // A door already closing must reopen rather than crush the player.
        w.level.doors[0].state = DoorState::Closing;
        w.level.doors[0].position = 0.5;
        w.update(1.0 / 60.0, &InputState::default());
        assert_eq!(
            w.level.doors[0].state,
            DoorState::Opening,
            "a closing door did not reopen for the player"
        );

        // The player must still be able to walk out of the doorway.
        let (px, py) = (w.player.x, w.player.y);
        for _ in 0..30 {
            w.update(1.0 / 60.0, &input_forward(1.0));
        }
        assert!(
            (w.player.x - px).abs() > 0.1 || (w.player.y - py).abs() > 0.1,
            "player got stuck in the doorway"
        );
    }

    /// Using a door must open it, never trigger the elevator switch. Door
    /// number 21 has tilemap byte `0x80 | 21`; masking off the high bits used
    /// to make it look like elevator-switch tile 21 and end the level.
    #[test]
    fn using_a_door_never_completes_the_level() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        // E1M1's door at (36,57) is door number 21, directly behind the door
        // the player starts facing.
        let idx = w
            .level
            .doors
            .iter()
            .position(|d| d.x == 36 && d.y == 57)
            .expect("door at 36,57");
        assert_eq!(idx, 21, "door numbering changed; pick another door");
        w.player.x = 35.5;
        w.player.y = 57.5;
        w.player.angle = 0.0; // face east, towards the door
        w.use_action();
        assert_eq!(
            w.state,
            PlayState::Playing,
            "using a door completed the level"
        );
        assert_ne!(
            w.level.doors[idx].state,
            DoorState::Closed,
            "the door did not open"
        );
    }

    #[test]
    fn death_spends_a_life_then_ends_the_game() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        w.player.lives = 1;
        w.state = PlayState::Died;
        assert!(!w.after_death(&data));
        assert_eq!(w.player.lives, 0);
        assert_eq!(w.state, PlayState::Playing);
        // With no lives left the next death is a game over.
        w.state = PlayState::Died;
        assert!(w.after_death(&data));
        assert_eq!(w.state, PlayState::GameOver);
    }

    #[test]
    fn intermission_bonus_rewards_100_percent() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        w.kill_count = w.kill_total;
        w.secret_count = w.level.secret_total;
        w.treasure_count = w.level.treasure_total;
        w.elapsed = 0.0;
        w.complete_level();
        assert_eq!(w.kill_percent(), 100);
        assert_eq!(w.secret_percent(), 100);
        assert_eq!(w.treasure_percent(), 100);
        assert!(w.last_bonus >= 30_000, "bonus too small: {}", w.last_bonus);
    }

    /// A pushable wall slides two tiles and opens the passage behind it.
    #[test]
    fn pushwall_slides_and_opens_a_passage() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        assert!(w.level.pushwalls.contains(&(10, 13)), "E1M1 pushwall moved");
        assert!(w.level.is_solid(10, 13));
        // Stand south of the wall and face it (north).
        w.player.x = 10.5;
        w.player.y = 14.5;
        w.player.angle = std::f32::consts::FRAC_PI_2;
        w.use_action();
        assert!(
            w.level.push_wall.as_ref().map(|p| p.active).unwrap_or(false),
            "the pushwall did not start moving"
        );
        assert_eq!(w.secret_count, 1, "the pushwall was not counted as a secret");
        for _ in 0..600 {
            w.update(1.0 / 60.0, &InputState::default());
        }
        assert!(
            !w.level.is_solid(10, 13),
            "the tile behind the pushwall is still solid"
        );
        assert!(
            w.level.is_solid(10, 11),
            "the pushwall did not come to rest two tiles away"
        );
    }

    /// The secret elevator leads to the secret floor, which returns to
    /// `ElevatorBackTo`.
    #[test]
    fn secret_elevator_routes_to_the_secret_floor() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        assert_eq!(w.level.tile(9, 51), ELEVATOR_TILE as u8, "switch moved");
        assert!(w.level.is_alt_elevator(10, 51), "secret floor moved");
        w.player.x = 10.5;
        w.player.y = 51.5;
        w.player.angle = std::f32::consts::PI; // face west, towards the switch
        w.use_action();
        assert_eq!(w.state, PlayState::LevelComplete);
        assert!(w.pending_secret, "secret elevator not detected");
        w.next_level(&data);
        assert_eq!(w.map_index, 9, "secret elevator did not reach the secret floor");
        assert_eq!(w.episode, 0);
        // Finishing the secret floor returns to ElevatorBackTo[0].
        w.complete_level();
        w.next_level(&data);
        assert_eq!(w.map_index, ELEVATOR_BACK_TO[0]);
        assert_eq!(w.episode, 0);
    }

    /// A normal elevator advances to the next floor, not the secret one.
    #[test]
    fn normal_elevator_advances_one_floor() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        w.player.x = 25.5;
        w.player.y = 47.5;
        w.player.angle = 0.0; // face east, towards the switch
        w.use_action();
        assert_eq!(w.state, PlayState::LevelComplete);
        assert!(!w.pending_secret);
        w.next_level(&data);
        assert_eq!(w.map_index, 1);
        assert_eq!(w.episode, 0);
    }

    /// The elevator switch must still end the level after the door fix.
    #[test]
    fn elevator_switch_completes_the_level() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        assert_eq!(
            w.level.tile(26, 47),
            crate::game::level::ELEVATOR_TILE as u8,
            "E1M1 elevator switch moved"
        );
        w.player.x = 25.5;
        w.player.y = 47.5;
        w.player.angle = 0.0; // face east, towards the switch
        w.use_action();
        assert_eq!(
            w.state,
            PlayState::LevelComplete,
            "the elevator switch did not end the level"
        );
    }

    #[test]
    fn firing_consumes_ammo() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        w.player.weapon = 1;
        w.player.ammo = 10;
        let before = w.player.ammo;
        let input = InputState {
            fire: true,
            fire_pressed: true,
            ..Default::default()
        };
        w.update(1.0 / 60.0, &input);
        assert!(w.player.ammo < before, "pistol did not consume ammo");
        assert!(w.player.attack_timer > 0.0, "no attack animation started");
    }

    #[test]
    fn damage_kills_an_actor() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        assert!(!w.actors.is_empty());
        w.damage_actor(0, 100_000);
        assert!(matches!(w.actors[0].state, ActorState::Dying | ActorState::Dead));
        assert!(w.player.score > 0);
    }

    #[test]
    fn doors_open_then_close() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        // Isolate the door from wandering enemies that would keep it open.
        w.actors.clear();
        let Some((idx, vertical, x, y)) = w
            .level
            .doors
            .iter()
            .enumerate()
            .map(|(i, d)| (i, d.vertical, d.x, d.y))
            .next()
        else {
            return;
        };
        // Stand next to the door facing it and use it.
        if vertical {
            w.player.x = x as f32 - 0.5;
            w.player.y = y as f32 + 0.5;
            w.player.angle = 0.0;
        } else {
            w.player.x = x as f32 + 0.5;
            w.player.y = y as f32 + 1.5;
            w.player.angle = std::f32::consts::FRAC_PI_2;
        }
        w.use_action();
        assert_ne!(w.level.doors[idx].state, DoorState::Closed);
        for _ in 0..120 {
            w.update(1.0 / 60.0, &InputState::default());
        }
        assert!(w.level.doors[idx].position > 0.9, "door did not open");
        // Wait out the auto-close timer plus closing time.
        for _ in 0..600 {
            w.update(1.0 / 60.0, &InputState::default());
        }
        assert!(w.level.doors[idx].position < 0.1, "door did not close");
    }

    #[test]
    fn items_can_be_picked_up() {
        let Some(data) = data() else { return };
        let mut w = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        let Some(idx) = w
            .level
            .statics
            .iter()
            .position(|s| s.active && s.item == Item::Clip)
        else {
            return;
        };
        w.player.ammo = 0;
        w.player.x = w.level.statics[idx].x as f32 + 0.5;
        w.player.y = w.level.statics[idx].y as f32 + 0.5;
        w.update(1.0 / 60.0, &InputState::default());
        assert!(!w.level.statics[idx].active, "clip was not picked up");
        assert!(w.player.ammo > 0, "ammo did not increase");
    }

    /// Stress every map with pseudo-random play (move, turn, fire, use) and
    /// make sure nothing panics.
    #[test]
    fn all_maps_survive_a_simulation() {
        let Some(data) = data() else { return };
        for i in 0..60 {
            let (episode, map) = (i / 10, i % 10);
            let mut w = World::new(&data, episode, map, Difficulty::Hard).unwrap();
            let mut seed = 0x9E37_79B9u32;
            let mut next = || {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed
            };
            for _ in 0..2500 {
                let f = (next() % 3) as f32 - 1.0;
                let s = (next() % 3) as f32 - 1.0;
                let t = (next() % 3) as f32 - 1.0;
                let fire = next() & 1 == 0;
                let use_pressed = next() % 7 == 0;
                let input = InputState {
                    forward: f,
                    strafe: s,
                    turn: t,
                    fire,
                    fire_pressed: fire,
                    use_pressed,
                    ..Default::default()
                };
                w.update(1.0 / 60.0, &input);
                if use_pressed {
                    w.use_action();
                }
                if w.state == PlayState::LevelComplete {
                    w.next_level(&data);
                } else if w.state == PlayState::Died {
                    w.restart_level(&data);
                }
            }
        }
    }

    #[test]
    fn all_sixty_maps_load() {
        let Some(data) = data() else { return };
        for i in 0..60 {
            let episode = i / 10;
            let map = i % 10;
            let w = World::new(&data, episode, map, Difficulty::Normal)
                .unwrap_or_else(|| panic!("map {i} failed to load"));
            assert_eq!((w.level.width, w.level.height), (64, 64));
        }
    }
}
