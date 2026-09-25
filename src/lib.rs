//! A clean-room reimplementation of Wolfenstein 3D built on the Bevy engine.
//!
//! The crate is split into:
//! * [`config`] — user configuration, including where the game data lives,
//! * [`data`] — decoders for the original WL6 data files,
//! * [`render`] — a software raycaster that reproduces the original's look,
//! * [`game`] — the game rules, actors and player logic,
//! * [`browser`] — the in-app game-folder picker,
//! * [`app`] — the Bevy app that ties everything together.
//!
//! SPDX-License-Identifier: MIT

pub mod android;
pub mod app;
pub mod config;
pub mod data;
pub mod game;
pub mod render;
pub mod touch;
