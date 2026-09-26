//! Level state: tiles, doors, static objects and the player start.
//!
//! SPDX-License-Identifier: MIT

use crate::data::{Map, generated::*};

/// First floor tile value; anything below is a wall.
pub const AREA_TILE: u16 = 107;
/// Tile used for the elevator switch.
pub const ELEVATOR_TILE: u16 = 21;
/// Floor tile that makes an elevator lead to the secret level.
pub const ALT_ELEVATOR_TILE: u16 = 107;
/// Plane-1 marker for a pushable wall.
pub const PUSHWALL_TILE: u16 = 98;
/// Plane-1 marker for the level exit.
pub const EXIT_TILE: u16 = 99;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DoorState {
    Closed,
    Opening,
    Open,
    Closing,
}

#[derive(Clone)]
pub struct Door {
    pub x: usize,
    pub y: usize,
    pub vertical: bool,
    /// 0 = normal, 1..=4 = locked, 5 = elevator.
    pub lock: u8,
    /// 0.0 closed .. 1.0 fully open.
    pub position: f32,
    pub state: DoorState,
    pub timer: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Dressing,
    Block,
    Gibs,
    Alpo,
    FirstAid,
    Key1,
    Key2,
    Key3,
    Key4,
    Cross,
    Chalice,
    Bible,
    Crown,
    Clip,
    Clip2,
    MachineGun,
    ChainGun,
    Food,
    FullHeal,
    Clip25,
    Spear,
}

impl Item {
    pub fn is_pickup(self) -> bool {
        !matches!(self, Item::Dressing | Item::Block)
    }
    pub fn blocks(self) -> bool {
        self == Item::Block
    }
    /// Treasure objects counted by the original's `treasuretotal`.
    pub fn is_treasure(self) -> bool {
        matches!(
            self,
            Item::Cross | Item::Chalice | Item::Bible | Item::Crown | Item::FullHeal
        )
    }
}

/// `statinfo[]` from the original: sprite and behaviour for each object value
/// (object value - 23). Indices beyond the table are ignored.
pub const STATINFO: [(u16, Item); 49] = [
    (SPR_STAT_0, Item::Dressing),
    (SPR_STAT_1, Item::Block),
    (SPR_STAT_2, Item::Block),
    (SPR_STAT_3, Item::Block),
    (SPR_STAT_4, Item::Dressing),
    (SPR_STAT_5, Item::Block),
    (SPR_STAT_6, Item::Alpo),
    (SPR_STAT_7, Item::Block),
    (SPR_STAT_8, Item::Block),
    (SPR_STAT_9, Item::Dressing),
    (SPR_STAT_10, Item::Block),
    (SPR_STAT_11, Item::Block),
    (SPR_STAT_12, Item::Block),
    (SPR_STAT_13, Item::Block),
    (SPR_STAT_14, Item::Dressing),
    (SPR_STAT_15, Item::Dressing),
    (SPR_STAT_16, Item::Block),
    (SPR_STAT_17, Item::Block),
    (SPR_STAT_18, Item::Block),
    (SPR_STAT_19, Item::Dressing),
    (SPR_STAT_20, Item::Key1),
    (SPR_STAT_21, Item::Key2),
    (SPR_STAT_22, Item::Block),
    (SPR_STAT_23, Item::Dressing),
    (SPR_STAT_24, Item::Food),
    (SPR_STAT_25, Item::FirstAid),
    (SPR_STAT_26, Item::Clip),
    (SPR_STAT_27, Item::MachineGun),
    (SPR_STAT_28, Item::ChainGun),
    (SPR_STAT_29, Item::Cross),
    (SPR_STAT_30, Item::Chalice),
    (SPR_STAT_31, Item::Bible),
    (SPR_STAT_32, Item::Crown),
    (SPR_STAT_33, Item::FullHeal),
    (SPR_STAT_34, Item::Gibs),
    (SPR_STAT_35, Item::Block),
    (SPR_STAT_36, Item::Block),
    (SPR_STAT_37, Item::Block),
    (SPR_STAT_38, Item::Gibs),
    (SPR_STAT_39, Item::Block),
    (SPR_STAT_40, Item::Block),
    (SPR_STAT_41, Item::Dressing),
    (SPR_STAT_42, Item::Dressing),
    (SPR_STAT_43, Item::Dressing),
    (SPR_STAT_44, Item::Dressing),
    (SPR_STAT_45, Item::Block),
    (SPR_STAT_46, Item::Block),
    (SPR_STAT_47, Item::Dressing),
    (SPR_STAT_26, Item::Clip2),
];

#[derive(Clone)]
pub struct Static {
    pub x: usize,
    pub y: usize,
    pub sprite: u16,
    pub item: Item,
    pub active: bool,
}

/// Direction a pushwall slides, in the map's `di_*` order (`east, north,
/// west, south`) used by the original's `PushWall`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PushDir {
    East,
    North,
    West,
    South,
}

impl PushDir {
    pub fn from_index(dir: usize) -> Self {
        match dir % 4 {
            0 => PushDir::East,
            1 => PushDir::North,
            2 => PushDir::West,
            _ => PushDir::South,
        }
    }

    /// Unit step in tile coordinates. `y` grows south.
    pub fn delta(self) -> (i32, i32) {
        match self {
            PushDir::East => (1, 0),
            PushDir::North => (0, -1),
            PushDir::West => (-1, 0),
            PushDir::South => (0, 1),
        }
    }

    /// Whether the wall's plane is perpendicular to x (slides east/west).
    pub fn vertical(self) -> bool {
        matches!(self, PushDir::East | PushDir::West)
    }
}

/// A wall that slides two tiles when the player pushes it (`PushWall` /
/// `MovePWalls` in the original). It is removed from the tile map as it
/// moves, opening a passage behind it.
#[derive(Clone)]
pub struct PushWall {
    /// Tile the sliding plane currently occupies.
    pub x: usize,
    pub y: usize,
    pub dir: PushDir,
    /// Progress within the current tile, `0.0..1.0`.
    pub pos: f32,
    /// Whole tiles already traversed, `0..=2`.
    pub moved: usize,
    /// Wall texture base value (`tile & 0x3F`).
    pub base: u8,
    pub active: bool,
}

impl PushWall {
    /// The coordinate of the sliding plane along its axis.
    pub fn plane(&self) -> f32 {
        let p = self.x as f32;
        let q = self.y as f32;
        match self.dir {
            PushDir::East => p + self.pos,
            PushDir::West => p + 1.0 - self.pos,
            PushDir::South => q + self.pos,
            PushDir::North => q + 1.0 - self.pos,
        }
    }

    /// The tile coordinate on the axis parallel to the wall (the wall spans
    /// one tile there).
    pub fn perp_tile(&self) -> usize {
        if self.dir.vertical() { self.y } else { self.x }
    }
}

pub struct Level {
    pub width: usize,
    pub height: usize,
    pub name: String,
    /// Wall tiles plus door encodings (`doornum | 0x80`) and door-side flags
    /// (`| 0x40`).
    pub tilemap: Vec<u8>,
    /// Plane 1, retained for pushwall/exit markers.
    pub objects: Vec<u16>,
    /// Blocking tiles contributed by static objects.
    pub blockers: Vec<bool>,
    pub doors: Vec<Door>,
    pub statics: Vec<Static>,
    pub player_start: (f32, f32, f32),
    pub exit_tiles: Vec<(usize, usize)>,
    /// Tiles marked with the pushable-wall object (98).
    pub pushwalls: Vec<(usize, usize)>,
    /// The single wall currently sliding, if any.
    pub push_wall: Option<PushWall>,
    /// `true` where the raw floor tile is `ALT_ELEVATOR_TILE` (107), i.e. the
    /// player standing there reaches the secret floor when using the elevator.
    pub alt_elevator: Vec<bool>,
    /// Number of pushwalls (the original's `secrettotal`).
    pub secret_total: usize,
    /// Number of treasure objects (the original's `treasuretotal`).
    pub treasure_total: usize,
    pub floor_color: u8,
    pub ceiling_color: u8,
    pub episode: usize,
    pub map_index: usize,
}

impl Level {
    #[inline]
    pub fn tile(&self, x: usize, y: usize) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.tilemap[y * self.width + x]
    }

    #[inline]
    pub fn is_door(&self, x: usize, y: usize) -> Option<usize> {
        if self.push_wall_at(x, y).is_some() {
            return None;
        }
        let t = self.tile(x, y);
        (t & 0x80 != 0).then_some((t & 0x7F) as usize)
    }

    /// The active pushwall, if its sliding tile is `(x, y)`.
    #[inline]
    pub fn push_wall_at(&self, x: usize, y: usize) -> Option<&PushWall> {
        match &self.push_wall {
            Some(pw) if pw.active && pw.x == x && pw.y == y => Some(pw),
            _ => None,
        }
    }

    /// Whether the elevator on this map leads to the secret floor from `(x, y)`.
    #[inline]
    pub fn is_alt_elevator(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.alt_elevator[y * self.width + x]
    }

    /// Whether a wall (solid tile or closed door) blocks movement here.
    pub fn is_solid(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return true;
        }
        let t = self.tilemap[y * self.width + x];
        if t & 0xC0 == 0xC0 {
            // An active pushwall is always solid while it slides.
            return true;
        }
        if t & 0x80 != 0 {
            // Door: solid until mostly open.
            let d = (t & 0x7F) as usize;
            return self.doors.get(d).map(|d| d.position < 0.9).unwrap_or(true);
        }
        // The `0x40` door-side flag is a rendering hint: a flagged floor tile
        // still has no collision (the original's `actorat` is 0 there).
        (t & 0x3F) != 0 || self.blockers[y * self.width + x]
    }

    /// Tile value used for texture selection (strips door-side flags).
    #[inline]
    pub fn wall_tile(&self, x: usize, y: usize) -> u8 {
        self.tile(x, y) & 0x3F
    }

    #[inline]
    pub fn object(&self, x: usize, y: usize) -> u16 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.objects[y * self.width + x]
    }
}

/// Build a playable [`Level`] from a decoded [`Map`].
pub fn build_level(map: &Map, episode: usize, map_index: usize) -> Level {
    let width = map.width;
    let height = map.height;
    let mut tilemap = vec![0u8; width * height];
    let mut blockers = vec![false; width * height];
    let mut objects = map.objects.clone();

    // Solid walls and floors.
    for i in 0..width * height {
        let t = map.walls[i];
        if t < AREA_TILE {
            tilemap[i] = t as u8;
        } else {
            tilemap[i] = 0;
        }
    }

    // Doors.
    let mut doors: Vec<Door> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let t = map.wall_at(x, y);
            if (90..=101).contains(&t) {
                let vertical = t % 2 == 0;
                let lock = ((t - 90) / 2) as u8;
                let doornum = doors.len();
                if doornum >= 64 {
                    continue;
                }
                doors.push(Door {
                    x,
                    y,
                    vertical,
                    lock,
                    position: 0.0,
                    state: DoorState::Closed,
                    timer: 0.0,
                });
                tilemap[y * width + x] = 0x80 | doornum as u8;
                // Mark the neighbouring tiles as door sides.
                if vertical {
                    if y > 0 {
                        tilemap[(y - 1) * width + x] |= 0x40;
                    }
                    if y + 1 < height {
                        tilemap[(y + 1) * width + x] |= 0x40;
                    }
                } else {
                    if x > 0 {
                        tilemap[y * width + x - 1] |= 0x40;
                    }
                    if x + 1 < width {
                        tilemap[y * width + x + 1] |= 0x40;
                    }
                }
            }
        }
    }

    // Static objects and markers.
    let mut statics = Vec::new();
    let mut player_start = (1.5, 1.5, 0.0);
    let mut exit_tiles = Vec::new();
    let mut pushwalls = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let o = map.object_at(x, y);
            match o {
                0 => {}
                19..=22 => {
                    let dir = o - 19; // 0 = north, 1 = east, 2 = south, 3 = west
                    let angle = match dir {
                        0 => std::f32::consts::FRAC_PI_2,
                        1 => 0.0,
                        2 => -std::f32::consts::FRAC_PI_2,
                        _ => std::f32::consts::PI,
                    };
                    player_start = (x as f32 + 0.5, y as f32 + 0.5, angle);
                }
                23..=74 => {
                    let ty = (o - 23) as usize;
                    if let Some(&(sprite, item)) = STATINFO.get(ty) {
                        if item.blocks() {
                            blockers[y * width + x] = true;
                        }
                        statics.push(Static {
                            x,
                            y,
                            sprite,
                            item,
                            active: true,
                        });
                    }
                }
                PUSHWALL_TILE => pushwalls.push((x, y)),
                EXIT_TILE => exit_tiles.push((x, y)),
                _ => {}
            }
        }
    }
    // Actors are spawned by the world; remove them from the object plane so
    // they are not treated as markers.
    for v in objects.iter_mut() {
        if (*v >= 108 && *v <= 259) || *v == 124 {
            *v = 0;
        }
    }

    let ceiling = CEILING_COLORS[(episode * 10 + map_index).min(CEILING_COLORS.len() - 1)];

    // Raw floor tile 107 marks the secret-elevator floor.
    let alt_elevator: Vec<bool> = map
        .walls
        .iter()
        .map(|&t| t == ALT_ELEVATOR_TILE)
        .collect();
    // The original counts pushwalls as secrets and treasure objects as treasure.
    let secret_total = pushwalls.len();
    let treasure_total = statics.iter().filter(|s| s.item.is_treasure()).count();

    Level {
        width,
        height,
        name: map.name.clone(),
        tilemap,
        objects,
        blockers,
        doors,
        statics,
        player_start,
        exit_tiles,
        pushwalls,
        push_wall: None,
        alt_elevator,
        secret_total,
        treasure_total,
        floor_color: 0x19,
        ceiling_color: ceiling,
        episode,
        map_index,
    }
}

/// Per-map ceiling colours (`vgaCeiling` from the original), low byte of each
/// 16-bit entry.
pub const CEILING_COLORS: [u8; 60] = [
    0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0xbf, //
    0x4e, 0x4e, 0x4e, 0x1d, 0x8d, 0x4e, 0x1d, 0x2d, 0x1d, 0x8d, //
    0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x2d, 0xdd, 0x1d, 0x1d, 0x98, //
    0x1d, 0x9d, 0x2d, 0xdd, 0xdd, 0x9d, 0x2d, 0x4d, 0x1d, 0xdd, //
    0x7d, 0x1d, 0x2d, 0x2d, 0xdd, 0xd7, 0x1d, 0x1d, 0x1d, 0x2d, //
    0x1d, 0x1d, 0x1d, 0x1d, 0xdd, 0xdd, 0x7d, 0xdd, 0xdd, 0xdd,
];

/// HUD picture indices re-exported for convenience.
pub use crate::data::vga::pic as hud_pic;
