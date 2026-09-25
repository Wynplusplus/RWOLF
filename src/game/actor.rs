//! Enemies: types, stats, sprite selection and spawn logic.
//!
//! SPDX-License-Identifier: MIT

use crate::data::generated::*;

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

    fn index(self) -> usize {
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

    /// Movement speed in tiles per second.
    pub fn speed(self) -> f32 {
        match self {
            ActorKind::Dog => 2.2,
            ActorKind::Officer | ActorKind::Mutant => 1.5,
            ActorKind::Guard | ActorKind::Ss => 1.1,
            ActorKind::Blinky | ActorKind::Clyde | ActorKind::Pinky | ActorKind::Inky => 1.4,
            _ => 0.8,
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
}

#[derive(Clone)]
pub struct Actor {
    pub x: f32,
    pub y: f32,
    /// Facing angle in radians (see the coordinate convention in `world`).
    pub facing: f32,
    pub kind: ActorKind,
    pub state: ActorState,
    pub health: i32,
    /// General-purpose state timer in seconds.
    pub timer: f32,
    /// Animation timer for walk/shoot cycles.
    pub anim: f32,
    pub walk_frame: usize,
    /// Patrol actors wander until they spot the player.
    pub patrol: bool,
    /// True once the actor has noticed the player.
    pub awake: bool,
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
            ActorState::Chase => {
                let base = kind.walk_sprite();
                base + (self.walk_frame as u16) * 8 + rotation(self.facing, view_angle)
            }
            ActorState::Idle => kind.stand_sprite() + rotation(self.facing, view_angle),
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
pub fn spawn_actors(
    width: usize,
    height: usize,
    objects: &[u16],
    difficulty: Difficulty,
) -> Vec<Actor> {
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
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut spawn = |kind: ActorKind, dir: u16, patrol: bool| {
                let facing = dir_angle(dir);
                out.push(Actor {
                    x: px,
                    y: py,
                    facing,
                    kind,
                    state: ActorState::Idle,
                    health: kind.health(difficulty),
                    timer: 0.0,
                    anim: 0.0,
                    walk_frame: 0,
                    patrol,
                    awake: false,
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

/// Map a `dir` (0 east, 1 north, 2 west, 3 south) to a facing angle.
///
/// Actor spawn directions in the map use the dirtype order of the original
/// (`east, north, west, south`), because `SpawnStand`/`SpawnPatrol` use
/// `new->dir = dir*2` to index the 8-way direction table.
pub fn dir_angle(dir: u16) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    match dir % 4 {
        0 => 0.0,         // east
        1 => FRAC_PI_2,   // north
        2 => PI,          // west
        _ => -FRAC_PI_2,  // south
    }
}
