//! Enemies: types, stats, sprite selection and spawn logic.
//!
//! SPDX-License-Identifier: MIT

use crate::data::generated::*;
use crate::game::level::Level;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActorKind {
    Guard,
    Officer,
    Ss,
    Dog,
    Mutant,
    Hans,
    Gretel,
    Gift,
    Fat,
    Schabbs,
    FakeHitler,
    Hitler,
    Blinky,
    Clyde,
    Pinky,
    Inky,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActorState {
    Idle,
    Chase,
    Shoot,
    /// The dog's leap-and-bite (`s_dogjump*`).
    Bite,
    Pain,
    Dying,
    Dead,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Baby,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub fn index(self) -> usize {
        match self {
            Difficulty::Baby => 0,
            Difficulty::Easy => 1,
            Difficulty::Normal => 2,
            Difficulty::Hard => 3,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Difficulty::Baby,
            1 => Difficulty::Easy,
            2 => Difficulty::Normal,
            _ => Difficulty::Hard,
        }
    }
}

impl ActorKind {
    pub fn health(self, d: Difficulty) -> i32 {
        // starthitpoints table from the original.
        let row = match d {
            Difficulty::Baby => 0,
            Difficulty::Easy => 1,
            Difficulty::Normal => 2,
            Difficulty::Hard => 3,
        };
        let table = [
            // baby
            [25, 50, 100, 1, 850, 850, 200, 800, 45, 25, 25, 25, 25, 850, 850, 850],
            // easy
            [25, 50, 100, 1, 950, 950, 300, 950, 55, 25, 25, 25, 25, 950, 950, 950],
            // normal
            [25, 50, 100, 1, 1050, 1550, 400, 1050, 55, 25, 25, 25, 25, 1050, 1050, 1050],
            // hard
            [25, 50, 100, 1, 1250, 1750, 500, 1250, 75, 25, 25, 25, 25, 1250, 1250, 1250],
        ];
        table[row][self.index()]
    }

    /// Stable index used by save games and the hit-point table.
    pub fn index(self) -> usize {
        match self {
            ActorKind::Guard => 0,
            ActorKind::Officer => 1,
            ActorKind::Ss => 2,
            ActorKind::Dog => 3,
            ActorKind::Hans => 4,
            ActorKind::Schabbs => 5,
            ActorKind::FakeHitler => 6,
            ActorKind::Hitler => 7,
            ActorKind::Mutant => 8,
            ActorKind::Blinky => 9,
            ActorKind::Clyde => 10,
            ActorKind::Pinky => 11,
            ActorKind::Inky => 12,
            ActorKind::Gretel => 13,
            ActorKind::Gift => 14,
            ActorKind::Fat => 15,
        }
    }

    /// Inverse of [`ActorKind::index`].
    pub fn from_index(i: usize) -> ActorKind {
        match i {
            0 => ActorKind::Guard,
            1 => ActorKind::Officer,
            2 => ActorKind::Ss,
            3 => ActorKind::Dog,
            4 => ActorKind::Hans,
            5 => ActorKind::Schabbs,
            6 => ActorKind::FakeHitler,
            7 => ActorKind::Hitler,
            8 => ActorKind::Mutant,
            9 => ActorKind::Blinky,
            10 => ActorKind::Clyde,
            11 => ActorKind::Pinky,
            12 => ActorKind::Inky,
            13 => ActorKind::Gretel,
            14 => ActorKind::Gift,
            _ => ActorKind::Fat,
        }
    }

    /// Idle/patrol speed in tiles per second (`SPDPATROL`/`SPDDOG`, converted
    /// from 16.16 tiles per tic at the original's ~70 tics per second).
    pub fn base_speed(self) -> f32 {
        match self {
            ActorKind::Dog => 1.60, // SPDDOG = 1500
            _ => 0.55,              // SPDPATROL = 512
        }
    }

    /// `FirstSighting` multiplies the speed when the actor starts chasing.
    pub fn chase_speed_mult(self) -> f32 {
        match self {
            ActorKind::Officer | ActorKind::Hitler => 5.0,
            ActorKind::Ss => 4.0,
            ActorKind::Guard
            | ActorKind::Mutant
            | ActorKind::Hans
            | ActorKind::Gretel
            | ActorKind::Gift
            | ActorKind::Fat
            | ActorKind::Schabbs
            | ActorKind::FakeHitler => 3.0,
            ActorKind::Dog
            | ActorKind::Blinky
            | ActorKind::Clyde
            | ActorKind::Pinky
            | ActorKind::Inky => 2.0,
        }
    }

    /// Damage dealt per successful attack.
    pub fn damage(self) -> i32 {
        match self {
            ActorKind::Guard => 4,
            ActorKind::Officer => 6,
            ActorKind::Ss => 8,
            ActorKind::Dog => 6,
            ActorKind::Mutant => 10,
            ActorKind::Blinky
            | ActorKind::Clyde
            | ActorKind::Pinky
            | ActorKind::Inky => 8,
            _ => 15,
        }
    }

    /// Distance (tiles) at which the actor will attack.
    pub fn attack_range(self) -> f32 {
        match self {
            ActorKind::Dog => 0.9,
            _ => 8.0,
        }
    }

    pub fn stand_sprite(self) -> u16 {
        match self {
            ActorKind::Guard => SPR_GRD_S_1,
            ActorKind::Officer => SPR_OFC_S_1,
            ActorKind::Ss => SPR_SS_S_1,
            ActorKind::Mutant => SPR_MUT_S_1,
            ActorKind::Dog => SPR_DOG_W1_1,
            ActorKind::Hans => SPR_BOSS_W1,
            ActorKind::Gretel => SPR_GRETEL_W1,
            ActorKind::Gift => SPR_GIFT_W1,
            ActorKind::Fat => SPR_FAT_W1,
            ActorKind::Schabbs => SPR_SCHABB_W1,
            ActorKind::FakeHitler => SPR_FAKE_W1,
            ActorKind::Hitler => SPR_HITLER_W1,
            ActorKind::Blinky => SPR_BLINKY_W1,
            ActorKind::Clyde => SPR_CLYDE_W1,
            ActorKind::Pinky => SPR_PINKY_W1,
            ActorKind::Inky => SPR_INKY_W1,
        }
    }

    pub fn walk_sprite(self) -> u16 {
        match self {
            ActorKind::Guard => SPR_GRD_W1_1,
            ActorKind::Officer => SPR_OFC_W1_1,
            ActorKind::Ss => SPR_SS_W1_1,
            ActorKind::Mutant => SPR_MUT_W1_1,
            ActorKind::Dog => SPR_DOG_W1_1,
            ActorKind::Hans => SPR_BOSS_W1,
            ActorKind::Gretel => SPR_GRETEL_W1,
            ActorKind::Gift => SPR_GIFT_W1,
            ActorKind::Fat => SPR_FAT_W1,
            ActorKind::Schabbs => SPR_SCHABB_W1,
            ActorKind::FakeHitler => SPR_FAKE_W1,
            ActorKind::Hitler => SPR_HITLER_W1,
            ActorKind::Blinky => SPR_BLINKY_W1,
            ActorKind::Clyde => SPR_CLYDE_W1,
            ActorKind::Pinky => SPR_PINKY_W1,
            ActorKind::Inky => SPR_INKY_W1,
        }
    }

    pub fn shoot_sprite(self) -> u16 {
        match self {
            ActorKind::Guard => SPR_GRD_SHOOT1,
            ActorKind::Officer => SPR_OFC_SHOOT1,
            ActorKind::Ss => SPR_SS_SHOOT1,
            ActorKind::Mutant => SPR_MUT_SHOOT1,
            ActorKind::Hans => SPR_BOSS_SHOOT1,
            ActorKind::Gretel => SPR_GRETEL_SHOOT1,
            ActorKind::Gift => SPR_GIFT_SHOOT1,
            ActorKind::Fat => SPR_FAT_SHOOT1,
            ActorKind::Schabbs => SPR_SCHABB_SHOOT1,
            ActorKind::FakeHitler => SPR_FAKE_SHOOT,
            ActorKind::Hitler => SPR_HITLER_SHOOT1,
            _ => 0,
        }
    }

    pub fn shoot_frames(self) -> u16 {
        match self {
            ActorKind::Mutant => 4,
            ActorKind::Dog => 0,
            ActorKind::Gift | ActorKind::Schabbs | ActorKind::FakeHitler => 2,
            _ => 3,
        }
    }

    /// The dog's bite animation (`s_dogjump1..3`).
    pub fn bite_sprite(self) -> u16 {
        match self {
            ActorKind::Dog => SPR_DOG_JUMP1,
            _ => 0,
        }
    }

    pub fn bite_frames(self) -> u16 {
        match self {
            ActorKind::Dog => 3,
            _ => 0,
        }
    }

    pub fn pain_sprite(self) -> u16 {
        match self {
            ActorKind::Guard => SPR_GRD_PAIN_1,
            ActorKind::Officer => SPR_OFC_PAIN_1,
            ActorKind::Ss => SPR_SS_PAIN_1,
            ActorKind::Mutant => SPR_MUT_PAIN_1,
            _ => 0,
        }
    }

    pub fn die_sprite(self) -> u16 {
        match self {
            ActorKind::Guard => SPR_GRD_DIE_1,
            ActorKind::Officer => SPR_OFC_DIE_1,
            ActorKind::Ss => SPR_SS_DIE_1,
            ActorKind::Mutant => SPR_MUT_DIE_1,
            ActorKind::Dog => SPR_DOG_DIE_1,
            ActorKind::Hans => SPR_BOSS_DIE1,
            ActorKind::Gretel => SPR_GRETEL_DIE1,
            ActorKind::Gift => SPR_GIFT_DIE1,
            ActorKind::Fat => SPR_FAT_DIE1,
            ActorKind::Schabbs => SPR_SCHABB_DIE1,
            ActorKind::FakeHitler => SPR_FAKE_DIE1,
            ActorKind::Hitler => SPR_HITLER_DIE1,
            _ => 0,
        }
    }

    pub fn die_frames(self) -> u16 {
        match self {
            ActorKind::Hitler => 7,
            ActorKind::Mutant => 4,
            ActorKind::Dog => 3,
            ActorKind::FakeHitler => 5,
            _ => 3,
        }
    }

    pub fn dead_sprite(self) -> u16 {
        match self {
            ActorKind::Guard => SPR_GRD_DEAD,
            ActorKind::Officer => SPR_OFC_DEAD,
            ActorKind::Ss => SPR_SS_DEAD,
            ActorKind::Mutant => SPR_MUT_DEAD,
            ActorKind::Dog => SPR_DOG_DEAD,
            ActorKind::Hans => SPR_BOSS_DEAD,
            ActorKind::Gretel => SPR_GRETEL_DEAD,
            ActorKind::Gift => SPR_GIFT_DEAD,
            ActorKind::Fat => SPR_FAT_DEAD,
            ActorKind::Schabbs => SPR_SCHABB_DEAD,
            ActorKind::FakeHitler => SPR_FAKE_DEAD,
            ActorKind::Hitler => SPR_HITLER_DEAD,
            _ => 0,
        }
    }

    pub fn sprite_base(self) -> u16 {
        self.stand_sprite()
    }

    /// Bosses gate the level's exit tile.
    pub fn is_boss(self) -> bool {
        matches!(
            self,
            ActorKind::Hans
                | ActorKind::Gretel
                | ActorKind::Gift
                | ActorKind::Fat
                | ActorKind::Schabbs
                | ActorKind::FakeHitler
                | ActorKind::Hitler
        )
    }

    /// Whether the walk/stand sprites have eight view rotations. Bosses and the
    /// Pac-Man ghosts use a single sprite per walk frame (`s_bosschase1` and
    /// `s_blinkychase1` have `rotate = false`), so adding a rotation offset
    /// would run into the next actor's sprites.
    pub fn rotates(self) -> bool {
        matches!(
            self,
            ActorKind::Guard
                | ActorKind::Officer
                | ActorKind::Ss
                | ActorKind::Mutant
                | ActorKind::Dog
        )
    }

    /// Number of walk frames per animation cycle.
    pub fn walk_frames(self) -> usize {
        match self {
            ActorKind::Blinky | ActorKind::Clyde | ActorKind::Pinky | ActorKind::Inky => 2,
            _ => 4,
        }
    }
}

/// The original's eight-way `dirtype` order, plus `NODIR`.
pub const NODIR: i32 = -1;

/// Unit step per `dirtype`, in tile coordinates (`y` grows south).
pub const DIR_DELTA: [(i32, i32); 8] = [
    (1, 0),   // east
    (1, -1),  // northeast
    (0, -1),  // north
    (-1, -1), // northwest
    (-1, 0),  // west
    (-1, 1),  // southwest
    (0, 1),   // south
    (1, 1),   // southeast
];

/// `opposite[]` from the original.
pub const OPPOSITE: [i32; 8] = [4, 5, 6, 7, 0, 1, 2, 3];

/// `diagonal[][]` from the original: the diagonal direction between two
/// cardinal directions, or `NODIR`.
pub fn diagonal(d1: i32, d2: i32) -> i32 {
    let (a, b) = (d1.min(d2), d1.max(d2));
    match (a, b) {
        (0, 2) => 1, // east + north -> northeast
        (0, 6) => 7, // east + south -> southeast
        (2, 4) => 3, // north + west -> northwest
        (4, 6) => 5, // west + south -> southwest
        _ => NODIR,
    }
}

/// The facing angle of a `dirtype` (0 = east, 2 = north, ...).
pub fn dir_angle8(dir: i32) -> f32 {
    if dir < 0 {
        0.0
    } else {
        (dir as f32) * std::f32::consts::TAU / 8.0
    }
}

#[derive(Clone)]
pub struct Actor {
    pub x: f32,
    pub y: f32,
    /// Destination tile the actor is walking to (`ob->tilex`/`tiley`).
    pub tile_x: i32,
    pub tile_y: i32,
    /// Current movement direction (`dirtype`, or `NODIR`).
    pub dir: i32,
    /// Distance left to the destination tile in tiles, or a negative door
    /// index when waiting for a door (`ob->distance`).
    pub distance: f32,
    /// Movement speed in tiles per second (`ob->speed`).
    pub speed: f32,
    /// Facing angle in radians for sprite selection.
    pub facing: f32,
    pub kind: ActorKind,
    pub state: ActorState,
    pub health: i32,
    /// General-purpose state timer in seconds.
    pub timer: f32,
    /// Animation timer for walk/shoot cycles.
    pub anim: f32,
    pub walk_frame: usize,
    /// Path actors follow the map's patrol arrows until they spot the player.
    pub patrol: bool,
    /// True once the actor has noticed the player.
    pub awake: bool,
    /// `FL_AMBUSH`: the actor only wakes when it sees the player, not on sound.
    pub ambush: bool,
    /// `FL_FIRSTATTACK`: the first dodge may turn around.
    pub first_attack: bool,
    /// Floor area number the actor stands in (`ob->areanumber`).
    pub area: usize,
    /// Set while an attack animation is resolving its shot.
    pub shot_fired: bool,
}

impl Actor {
    pub fn is_alive(&self) -> bool {
        !matches!(self.state, ActorState::Dying | ActorState::Dead)
    }

    /// The sprite to draw for the current state and view angle.
    pub fn sprite(&self, view_angle: f32) -> Option<u16> {
        let kind = self.kind;
        let s = match self.state {
            ActorState::Dead => kind.dead_sprite(),
            ActorState::Dying => {
                let base = kind.die_sprite();
                if base == 0 {
                    return None;
                }
                let f = (self.timer / 0.18) as u16;
                base + f.min(kind.die_frames().saturating_sub(1))
            }
            ActorState::Pain => {
                let p = kind.pain_sprite();
                if p == 0 { kind.stand_sprite() } else { p }
            }
            ActorState::Shoot => {
                let f = (self.timer / 0.12) as u16;
                let base = kind.shoot_sprite();
                if base == 0 {
                    kind.stand_sprite()
                } else {
                    base + f.min(kind.shoot_frames().saturating_sub(1))
                }
            }
            ActorState::Bite => {
                let base = kind.bite_sprite();
                if base == 0 {
                    kind.stand_sprite()
                } else {
                    let f = (self.timer / 0.14) as u16;
                    base + f.min(kind.bite_frames().saturating_sub(1))
                }
            }
            ActorState::Chase => {
                let base = kind.walk_sprite();
                let frame = self.walk_frame % kind.walk_frames();
                if kind.rotates() {
                    base + (frame as u16) * 8 + rotation(self.facing, view_angle)
                } else {
                    base + frame as u16
                }
            }
            ActorState::Idle => {
                let base = kind.stand_sprite();
                if kind.rotates() {
                    base + rotation(self.facing, view_angle)
                } else {
                    base
                }
            }
        };
        (s != 0).then_some(s)
    }
}

/// Pick one of eight sprite rotations for an actor facing `facing` when viewed
/// from `view_angle` (the player's look direction). Mirrors `CalcRotate`.
pub fn rotation(facing: f32, view_angle: f32) -> u16 {
    let tau = std::f32::consts::TAU;
    let mut a = (view_angle - std::f32::consts::PI) - facing + tau / 16.0;
    a = a.rem_euclid(tau);
    ((a / (tau / 8.0)) as u16) % 8
}

/// Spawn the actors encoded in a map's object plane for `difficulty`.
///
/// This mirrors `ScanInfoPlane`, including the difficulty gates.
pub fn spawn_actors(level: &Level, objects: &[u16], difficulty: Difficulty) -> Vec<Actor> {
    let width = level.width;
    let height = level.height;
    let mut out = Vec::new();
    let allow = |d: Difficulty| -> (bool, bool) {
        // (medium-and-up allowed, hard-only allowed)
        match d {
            Difficulty::Baby | Difficulty::Easy => (false, false),
            Difficulty::Normal => (true, false),
            Difficulty::Hard => (true, true),
        }
    };
    let (medium, hard) = allow(difficulty);

    for y in 0..height {
        for x in 0..width {
            let o = objects[y * width + x];
            if o == 0 {
                continue;
            }
            let idx = y * width + x;
            let area = level.areas[idx] as usize;
            let ambush = level.ambush[idx];
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut spawn = |kind: ActorKind, map_dir: u16, patrol: bool| {
                let dir = (map_dir as i32) * 2;
                out.push(Actor {
                    x: px,
                    y: py,
                    tile_x: x as i32,
                    tile_y: y as i32,
                    dir,
                    distance: 0.0,
                    speed: kind.base_speed(),
                    facing: dir_angle8(dir),
                    kind,
                    state: ActorState::Idle,
                    health: kind.health(difficulty),
                    timer: 0.0,
                    anim: 0.0,
                    walk_frame: 0,
                    patrol,
                    awake: false,
                    ambush,
                    first_attack: false,
                    area,
                    shot_fired: false,
                });
            };

            match o {
                // Guard stand / patrol, with medium and hard upgrades.
                108..=111 => spawn(ActorKind::Guard, o - 108, false),
                112..=115 => spawn(ActorKind::Guard, o - 112, true),
                144..=147 if medium => spawn(ActorKind::Guard, o - 144, false),
                148..=151 if medium => spawn(ActorKind::Guard, o - 148, true),
                180..=183 if hard => spawn(ActorKind::Guard, o - 180, false),
                184..=187 if hard => spawn(ActorKind::Guard, o - 184, true),
                // Officer.
                116..=119 => spawn(ActorKind::Officer, o - 116, false),
                120..=123 => spawn(ActorKind::Officer, o - 120, true),
                152..=155 if medium => spawn(ActorKind::Officer, o - 152, false),
                156..=159 if medium => spawn(ActorKind::Officer, o - 156, true),
                188..=191 if hard => spawn(ActorKind::Officer, o - 188, false),
                192..=195 if hard => spawn(ActorKind::Officer, o - 192, true),
                // SS.
                126..=129 => spawn(ActorKind::Ss, o - 126, false),
                130..=133 => spawn(ActorKind::Ss, o - 130, true),
                162..=165 if medium => spawn(ActorKind::Ss, o - 162, false),
                166..=169 if medium => spawn(ActorKind::Ss, o - 166, true),
                198..=201 if hard => spawn(ActorKind::Ss, o - 198, false),
                202..=205 if hard => spawn(ActorKind::Ss, o - 202, true),
                // Dogs.
                134..=137 => spawn(ActorKind::Dog, o - 134, false),
                138..=141 => spawn(ActorKind::Dog, o - 138, true),
                170..=173 if medium => spawn(ActorKind::Dog, o - 170, false),
                174..=177 if medium => spawn(ActorKind::Dog, o - 174, true),
                206..=209 if hard => spawn(ActorKind::Dog, o - 206, false),
                210..=213 if hard => spawn(ActorKind::Dog, o - 210, true),
                // Mutants.
                216..=219 => spawn(ActorKind::Mutant, o - 216, false),
                220..=223 => spawn(ActorKind::Mutant, o - 220, true),
                234..=237 if medium => spawn(ActorKind::Mutant, o - 234, false),
                238..=241 if medium => spawn(ActorKind::Mutant, o - 238, true),
                252..=255 if hard => spawn(ActorKind::Mutant, o - 252, false),
                256..=259 if hard => spawn(ActorKind::Mutant, o - 256, true),
                // Bosses don't use the map direction in the original; they
                // start facing a fixed direction (mostly south). They wake and
                // chase the player immediately, so this is largely transient.
                214 => spawn(ActorKind::Hans, 3, false),
                197 => spawn(ActorKind::Gretel, 1, false),
                215 => spawn(ActorKind::Gift, 3, false),
                179 => spawn(ActorKind::Fat, 1, false),
                196 => spawn(ActorKind::Schabbs, 3, false),
                160 => spawn(ActorKind::FakeHitler, 3, false),
                178 => spawn(ActorKind::Hitler, 3, false),
                224 => spawn(ActorKind::Blinky, 1, false),
                225 => spawn(ActorKind::Clyde, 1, false),
                226 => spawn(ActorKind::Pinky, 1, false),
                227 => spawn(ActorKind::Inky, 1, false),
                _ => {}
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(kind: ActorKind, state: ActorState, walk_frame: usize) -> Actor {
        Actor {
            x: 1.5,
            y: 1.5,
            tile_x: 1,
            tile_y: 1,
            dir: 0,
            distance: 0.0,
            speed: kind.base_speed(),
            facing: 0.0,
            kind,
            state,
            health: 100,
            timer: 0.0,
            anim: 0.0,
            walk_frame,
            patrol: false,
            awake: true,
            ambush: false,
            first_attack: false,
            area: 0,
            shot_fired: false,
        }
    }

    /// Bosses use one sprite per walk frame, so their walk animation must stay
    /// inside the four walk sprites instead of spilling into the shoot frames.
    #[test]
    fn boss_walk_sprites_stay_in_range() {
        let bosses = [
            (ActorKind::Hans, SPR_BOSS_W1),
            (ActorKind::Gretel, SPR_GRETEL_W1),
            (ActorKind::Gift, SPR_GIFT_W1),
            (ActorKind::Fat, SPR_FAT_W1),
            (ActorKind::Schabbs, SPR_SCHABB_W1),
            (ActorKind::FakeHitler, SPR_FAKE_W1),
            (ActorKind::Hitler, SPR_HITLER_W1),
        ];
        for (kind, base) in bosses {
            assert!(!kind.rotates());
            for frame in 0..4 {
                let a = actor(kind, ActorState::Chase, frame);
                let s = a.sprite(0.0).unwrap();
                assert!(
                    s >= base && s < base + 4,
                    "{kind:?} frame {frame} sprite {s} outside {base}..{}",
                    base + 4
                );
            }
            // Idle must also stay on the first walk sprite.
            assert_eq!(actor(kind, ActorState::Idle, 0).sprite(0.0).unwrap(), base);
        }
    }

    /// The Pac-Man ghosts also have no rotations and only two walk frames.
    #[test]
    fn ghost_walk_sprites_stay_in_range() {
        let ghosts = [
            (ActorKind::Blinky, SPR_BLINKY_W1),
            (ActorKind::Pinky, SPR_PINKY_W1),
            (ActorKind::Clyde, SPR_CLYDE_W1),
            (ActorKind::Inky, SPR_INKY_W1),
        ];
        for (kind, base) in ghosts {
            assert!(!kind.rotates());
            assert_eq!(kind.walk_frames(), 2);
            for frame in 0..2 {
                assert_eq!(
                    actor(kind, ActorState::Chase, frame).sprite(0.0).unwrap(),
                    base + frame as u16
                );
            }
        }
    }

    /// Rotating actors keep eight sprites per walk frame.
    #[test]
    fn guard_walk_sprites_rotate() {
        for frame in 0..4 {
            let a = actor(ActorKind::Guard, ActorState::Chase, frame);
            for (i, angle) in [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0].into_iter().enumerate() {
                let s = a.sprite(angle).unwrap();
                let offset = s - SPR_GRD_W1_1;
                assert!(
                    offset >= frame as u16 * 8 && offset < (frame as u16 + 1) * 8,
                    "guard frame {frame} angle {i} sprite {s} out of its 8 rotations"
                );
            }
        }
    }
}
