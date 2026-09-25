//! Game rules: player movement, combat, doors, items and enemy AI.
//!
//! SPDX-License-Identifier: MIT

use crate::data::generated::*;
use crate::data::{GameData, Map};
use crate::game::actor::{Actor, ActorKind, ActorState, Difficulty, dir_angle, spawn_actors};
use crate::game::hud::Hud;
use crate::game::level::{DoorState, Item, Level, build_level};

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
            self.sounds.push(32); // SHOOTSND (wall impact)
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
                a.walk_frame = (a.walk_frame + 1) % 4;
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
            self.move_actor(i, mvx, mvy);
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
            a.walk_frame = (a.walk_frame + 1) % 4;
        }
    }

    fn move_actor(&mut self, i: usize, dx: f32, dy: f32) {
        self.try_move_actor(i, dx, dy);
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
        if tx < self.level.width && ty < self.level.height {
            let obj = self.level.object(tx, ty);
            if obj == crate::game::level::EXIT_TILE {
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

        // Elevator switch.
        if east_west
            && self.level.wall_tile(ux, uy) == crate::game::level::ELEVATOR_TILE as u8
        {
            let secret = self.level.wall_tile(
                self.player.x.floor() as usize,
                self.player.y.floor() as usize,
            ) == crate::game::level::ALT_ELEVATOR_TILE as u8;
            self.level.secret_floor = secret;
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
        self.state = PlayState::LevelComplete;
        self.transition_timer = 0.0;
    }

    /// Advance to the next map, keeping the player's stats.
    pub fn next_level(&mut self, data: &GameData) {
        let mut next = self.map_index + 1;
        let mut episode = self.episode;
        if next >= 10 {
            next = 0;
            episode = (episode + 1) % 6;
        }
        if let Some(map) = data.maps.get(episode * 10 + next) {
            let level = build_level(map, episode, next);
            let actors = spawn_actors(level.width, level.height, &map.objects, self.difficulty);
            let (px, py, pa) = level.player_start;
            self.level = level;
            self.actors = actors;
            self.player.x = px;
            self.player.y = py;
            self.player.angle = pa;
            self.player.attack_timer = 0.0;
            self.player.damage_flash = 0.0;
            self.map_index = next;
            self.episode = episode;
            self.state = PlayState::Playing;
            self.transition_timer = 0.0;
            self.sync_hud();
        }
    }

    /// Restart the current level after death.
    pub fn restart_level(&mut self, data: &GameData) {
        if self.player.lives > 0 {
            self.player.lives -= 1;
        }
        self.player.health = 100;
        self.player.ammo = (self.player.ammo).max(8);
        if let Some(map) = data.maps.get(self.episode * 10 + self.map_index) {
            let level = build_level(map, self.episode, self.map_index);
            let actors = spawn_actors(level.width, level.height, &map.objects, self.difficulty);
            let (px, py, pa) = level.player_start;
            self.level = level;
            self.actors = actors;
            self.player.x = px;
            self.player.y = py;
            self.player.angle = pa;
            self.player.attack_timer = 0.0;
            self.player.damage_flash = 0.0;
            self.state = PlayState::Playing;
            self.transition_timer = 0.0;
            self.sync_hud();
        }
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
