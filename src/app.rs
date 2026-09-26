//! Bevy integration: window, input, the game loop and screen upload.
//!
//! SPDX-License-Identifier: MIT

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings};
use bevy::camera::ScalingMode;
use bevy::ecs::schedule::common_conditions::{not, resource_exists};
use bevy::image::ImageSampler;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{CursorGrabMode, CursorOptions, WindowResolution};
#[cfg(target_os = "android")]
use bevy::window::{MonitorSelection, WindowMode};

use crate::data::{GameData, audio::SAMPLE_RATE};
use crate::game::actor::Difficulty;
use crate::game::browser::{Browser, inside, layout};
use crate::game::savegame;
use crate::game::scores::HighScores;
use crate::game::world::{InputState, PlayState, World};
use crate::render::framebuffer::{Framebuffer, VIEW_H, VIEW_W};
use crate::render::hud::{
    EPISODE_MAPS, EPISODES, Intermission, MenuHit, draw_intermission, draw_level_select,
    draw_status_bar, menu_hit,
};
use crate::touch::{TouchControls, draw_controls, read_touch};
use crate::render::raycast::{
    Camera, collect_sprites, render_sprites, render_walls, render_weapon,
};

const SCALE: usize = 2;

#[derive(Resource)]
struct DataRes(GameData);

#[derive(Resource)]
struct WorldRes(World);

#[derive(Resource, Default)]
struct InputRes(InputState);

#[derive(Resource)]
struct Screen {
    image: Handle<Image>,
    fb: Framebuffer,
    rgba: Vec<u8>,
    zbuf: [f32; VIEW_W],
}

#[derive(Resource)]
struct SoundBank {
    handles: Vec<Handle<AudioSource>>,
}

/// Persistent high-score table.
#[derive(Resource)]
struct ScoresRes(HighScores);

#[derive(Resource, Default)]
struct MouseCaptured(bool);

/// The `Escape` level-select overlay. While open the game is paused.
#[derive(Resource)]
struct LevelMenu {
    open: bool,
    /// Zero-based episode/map selection.
    episode: usize,
    map: usize,
    /// Zero-based difficulty selection (see `Difficulty`).
    difficulty: usize,
    /// The high-score table is covering the menu.
    show_scores: bool,
}

impl Default for LevelMenu {
    fn default() -> Self {
        Self {
            open: false,
            episode: 0,
            map: 0,
            difficulty: Difficulty::Normal.index(),
            show_scores: false,
        }
    }
}

#[derive(Resource, Default)]
struct ScreenshotState {
    frames: u32,
    done: bool,
}

pub fn run() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Wolfenstein 3D (Bevy reimplementation)".into(),
                resolution: WindowResolution::new((VIEW_W * SCALE) as u32, (VIEW_H * SCALE) as u32),
                // Phones get the whole screen; there is no window chrome and
                // the touch overlay assumes it can use the full surface.
                #[cfg(target_os = "android")]
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<MouseCaptured>()
        .init_resource::<ScreenshotState>()
        .init_resource::<LevelMenu>()
        .init_resource::<TouchControls>()
        .init_resource::<InputRes>()
        .init_resource::<Browser>()
        .add_systems(Startup, (setup, capture_cursor))
        .add_systems(
            Update,
            (
                read_input,
                read_touch,
                apply_touch,
                browse_input,
                handle_level_menu.run_if(resource_exists::<DataRes>),
                update_world.run_if(game_running),
                play_sounds.run_if(game_running),
                handle_transitions.run_if(game_running),
                load_selected_dir,
                render_browser.run_if(browser_open),
                render_world
                    .run_if(resource_exists::<DataRes>)
                    .run_if(not(browser_open)),
                maybe_screenshot,
            )
                .chain(),
        )
        .run()
}

/// True while the folder picker is on screen.
fn browser_open(browser: Res<Browser>) -> bool {
    browser.open
}

/// True while the game should be simulated: data is loaded and no overlay is
/// open.
fn game_running(
    menu: Res<LevelMenu>,
    browser: Res<Browser>,
    data: Option<Res<DataRes>>,
) -> bool {
    data.is_some() && !menu.open && !browser.open
}

/// Dev helper: when `WOLF3D_SCREENSHOT` is set, save a PNG of the window after
/// a short warm-up. Used to verify the GPU presentation path.
fn maybe_screenshot(mut commands: Commands, mut state: ResMut<ScreenshotState>) {
    let Ok(path) = std::env::var("WOLF3D_SCREENSHOT") else {
        return;
    };
    if state.done {
        return;
    }
    state.frames += 1;
    if state.frames == 90 {
        state.done = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut browser: ResMut<Browser>,
) {
    if let Some(src) = &crate::config::get().source {
        info!("Using config {}", src.display());
    }
    let scores_path = crate::config::persist_path("wolf3d-bevy.scores");
    commands.insert_resource(ScoresRes(HighScores::load(&scores_path)));
    // On Android this creates the directory the user pushes their data into.
    crate::data::prepare_storage();

    // The framebuffer, camera and sprite always exist so a diagnostic screen
    // can be drawn before (or instead of) the game.
    let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
    fb.clear(0);
    let rgba = vec![0u8; VIEW_W * VIEW_H * 4];
    let image = images.add(make_image(&rgba));
    let image_handle = image.clone();
    commands.insert_resource(Screen {
        image,
        fb,
        rgba,
        zbuf: [f32::INFINITY; VIEW_W],
    });

    // 2D camera sized so the whole 320x200 framebuffer is always visible.
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: VIEW_W as f32,
                min_height: VIEW_H as f32,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
    commands.spawn((
        Sprite {
            image: image_handle,
            custom_size: Some(Vec2::new(VIEW_W as f32, VIEW_H as f32)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Try to start the game; otherwise open the folder picker.
    if let Some(dir) = crate::data::find_data_dir() {
        match GameData::load(&dir) {
            Ok(data) => {
                info!("Loaded Wolfenstein 3D data from {}", dir.display());
                let episode = start_episode();
                let map = start_map();
                let difficulty = start_difficulty();
                info!(
                    "Starting episode {} map {} (difficulty {:?})",
                    episode + 1,
                    map + 1,
                    difficulty
                );
                install_game(
                    &mut commands,
                    &mut audio_sources,
                    data,
                    episode,
                    map,
                    difficulty,
                );
                return;
            }
            Err(e) => {
                error!("Failed to load game data from {}: {e}", dir.display());
                browser.open_at(dir);
                browser.status = format!("FAILED TO LOAD: {e}");
                return;
            }
        }
    }

    error!(
        "Could not find Wolfenstein 3D data. Pick your WL6 folder in the app, \
         set WOLF3D_DATA_DIR, or set `data_dir` in wolf3d-bevy.toml."
    );
    // On Android, ask for storage access up front so the picker can read
    // shared storage such as Download.
    if !crate::android::granted() {
        crate::android::request();
        browser.access_requested = true;
    }
    browser.open_default();
    browser.status = "NO GAME DATA - PICK YOUR WOLF3D FOLDER".to_string();
}

/// Build the sound bank, create the world and install the game resources.
fn install_game(
    commands: &mut Commands,
    audio_sources: &mut Assets<AudioSource>,
    data: GameData,
    episode: usize,
    map: usize,
    difficulty: Difficulty,
) {
    // Sound bank: wrap each PCM chunk in a WAV container.
    let mut handles = Vec::with_capacity(data.audio.count());
    for i in 0..data.audio.count() {
        let samples = data.audio.sound_i16(i);
        let wav = wav_from_i16(&samples, SAMPLE_RATE);
        handles.push(audio_sources.add(AudioSource {
            bytes: Arc::from(wav.into_boxed_slice()),
        }));
    }

    let world = World::new(&data, episode, map, difficulty).expect("requested map should exist");
    commands.insert_resource(DataRes(data));
    commands.insert_resource(WorldRes(world));
    commands.insert_resource(SoundBank { handles });
    commands.insert_resource(LevelMenu {
        open: false,
        episode,
        map,
        difficulty: difficulty.index(),
        show_scores: false,
    });
}

fn make_image(rgba: &[u8]) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: VIEW_W as u32,
            height: VIEW_H as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn capture_cursor(mut cursor: Query<&mut CursorOptions, With<Window>>) {
    if let Ok(mut c) = cursor.single_mut() {
        c.grab_mode = CursorGrabMode::Locked;
        c.visible = false;
    }
}

/// Starting episode (zero-based). `WOLF3D_EPISODE` overrides the config file.
fn start_episode() -> usize {
    std::env::var("WOLF3D_EPISODE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .or(crate::config::get().episode)
        .map(|e| e.saturating_sub(1).min(EPISODES - 1))
        .unwrap_or(0)
}

/// Starting map (zero-based). `WOLF3D_MAP` overrides the config file.
fn start_map() -> usize {
    std::env::var("WOLF3D_MAP")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .or(crate::config::get().map)
        .map(|m| m.saturating_sub(1).min(EPISODE_MAPS - 1))
        .unwrap_or(0)
}

/// Starting difficulty. `WOLF3D_DIFFICULTY` overrides the config file.
fn start_difficulty() -> Difficulty {
    let name = std::env::var("WOLF3D_DIFFICULTY")
        .ok()
        .or_else(|| crate::config::get().difficulty.clone())
        .unwrap_or_default();
    match name.to_ascii_lowercase().as_str() {
        "baby" => Difficulty::Baby,
        "easy" => Difficulty::Easy,
        "hard" => Difficulty::Hard,
        _ => Difficulty::Normal,
    }
}

fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse: Res<AccumulatedMouseMotion>,
    mut input: ResMut<InputRes>,
) {
    let i = &mut input.0;
    *i = InputState::default();

    let mut forward = 0.0;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        forward += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        forward -= 1.0;
    }
    i.forward = forward;

    let mut strafe = 0.0;
    if keys.pressed(KeyCode::KeyA) {
        strafe -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        strafe += 1.0;
    }
    i.strafe = strafe;

    // `update_player` does `angle -= turn * speed`, and increasing `angle`
    // rotates counter-clockwise (left). So left must be negative and right
    // positive; the previous signs made Q/ArrowLeft steer right.
    let mut turn = 0.0;
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyQ) {
        turn -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyE) {
        turn += 1.0;
    }
    i.turn = turn;
    i.mouse_dx = mouse.delta.x;
    i.run = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    i.fire = buttons.pressed(MouseButton::Left) || keys.pressed(KeyCode::ControlLeft);
    i.fire_pressed = buttons.just_pressed(MouseButton::Left) || keys.just_pressed(KeyCode::ControlLeft);
    i.use_pressed = keys.just_pressed(KeyCode::Space);

    if keys.just_pressed(KeyCode::Digit1) {
        i.next_weapon = Some(0);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        i.next_weapon = Some(1);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        i.next_weapon = Some(2);
    }
    if keys.just_pressed(KeyCode::Digit4) {
        i.next_weapon = Some(3);
    }
}

/// Merge touch input into this frame's [`InputState`]. Keyboard/mouse input
/// already gathered by [`read_input`] is preserved.
fn apply_touch(
    menu: Res<LevelMenu>,
    browser: Res<Browser>,
    controls: Res<TouchControls>,
    mut input: ResMut<InputRes>,
) {
    if !controls.enabled || menu.open || browser.open {
        return;
    }
    let i = &mut input.0;
    i.forward += controls.movement.y;
    i.turn += controls.movement.x;
    i.mouse_dx += controls.look_dx;
    i.fire |= controls.fire;
    i.fire_pressed |= controls.fire_pressed;
    i.use_pressed |= controls.use_pressed;
    i.run |= controls.run;
    if i.next_weapon.is_none() {
        i.next_weapon = controls.weapon;
    }
}

/// Navigate the game-folder picker. Runs every frame but only acts while it is
/// open. Taps come from the touch overlay; the keyboard works on desktop.
fn browse_input(
    keys: Res<ButtonInput<KeyCode>>,
    controls: Res<TouchControls>,
    time: Res<Time>,
    data: Option<Res<DataRes>>,
    mut refresh_timer: Local<f32>,
    mut browser: ResMut<Browser>,
) {
    if !browser.open {
        return;
    }

    // Storage permission: ask once when a folder could not be read, then
    // re-check periodically so the picker refreshes after the user returns
    // from the Android settings page.
    if browser.access_error {
        if !browser.access_requested {
            browser.access_requested = true;
            if !crate::android::granted() {
                crate::android::request();
            }
        }
        *refresh_timer -= time.delta_secs();
        if *refresh_timer <= 0.0 {
            *refresh_timer = 1.0;
            if crate::android::granted() {
                browser.refresh();
            }
        }
    }

    if keys.just_pressed(KeyCode::ArrowDown) {
        browser.select_delta(1);
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        browser.select_delta(-1);
    }
    if keys.just_pressed(KeyCode::PageDown) {
        browser.scroll_delta(layout::VISIBLE_ROWS as i32);
    }
    if keys.just_pressed(KeyCode::PageUp) {
        browser.scroll_delta(-(layout::VISIBLE_ROWS as i32));
    }
    if keys.just_pressed(KeyCode::Enter) {
        browser.enter_selected();
    }
    if keys.just_pressed(KeyCode::Backspace) {
        browser.go_up();
    }
    if keys.just_pressed(KeyCode::KeyR) {
        browser.next_root();
    }
    if keys.just_pressed(KeyCode::KeyU) {
        confirm_browser(&mut browser);
    }
    if keys.just_pressed(KeyCode::Escape) && data.is_some() {
        browser.open = false;
    }

    if let Some(p) = controls.tap {
        let (x, y) = (p.x, p.y);
        if browser.access_error && inside(layout::grant_rect(), x, y) {
            if crate::android::granted() {
                browser.refresh();
            } else {
                crate::android::request();
            }
        } else if inside(layout::close_rect(), x, y) {
            if data.is_some() {
                browser.open = false;
            }
        } else if inside(layout::up_rect(), x, y) {
            browser.go_up();
        } else if inside(layout::roots_rect(), x, y) {
            browser.next_root();
        } else if inside(layout::use_rect(), x, y) {
            confirm_browser(&mut browser);
        } else if inside(layout::scroll_up_rect(), x, y) {
            browser.scroll_delta(-1);
        } else if inside(layout::scroll_down_rect(), x, y) {
            browser.scroll_delta(1);
        } else {
            for i in 0..layout::VISIBLE_ROWS {
                if inside(layout::row_rect(i), x, y) {
                    let index = browser.scroll + i;
                    if index < browser.entries.len() {
                        browser.selected = index;
                        browser.enter_selected();
                    }
                    break;
                }
            }
        }
    }
}

fn confirm_browser(browser: &mut Browser) {
    if let Some(dir) = browser.confirm() {
        crate::config::set_data_dir(dir.clone());
        browser.pending = Some(dir);
    }
}

/// Load the folder the user confirmed in the picker.
fn load_selected_dir(
    mut commands: Commands,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut browser: ResMut<Browser>,
) {
    let Some(dir) = browser.pending.take() else {
        return;
    };
    match GameData::load(&dir) {
        Ok(data) => {
            info!("Loaded Wolfenstein 3D data from {}", dir.display());
            install_game(
                &mut commands,
                &mut audio_sources,
                data,
                start_episode(),
                start_map(),
                start_difficulty(),
            );
            browser.open = false;
        }
        Err(e) => {
            error!("Failed to load game data from {}: {e}", dir.display());
            browser.status = format!("FAILED TO LOAD: {e}");
        }
    }
}

/// Draw the folder picker over the framebuffer.
fn render_browser(
    browser: Res<Browser>,
    data: Option<Res<DataRes>>,
    mut screen: ResMut<Screen>,
    mut images: ResMut<Assets<Image>>,
) {
    if !browser.open {
        return;
    }
    let screen = &mut *screen;
    crate::render::browser_ui::draw(&mut screen.fb, &browser, data.is_some());
    screen.fb.to_rgba(&mut screen.rgba);
    upload_screen(screen, &mut images);
}

/// Upload the framebuffer's RGBA data to the sprite's image.
/// Callers must have refreshed `screen.rgba` with [`Framebuffer::to_rgba`].
fn upload_screen(screen: &mut Screen, images: &mut Assets<Image>) {
    if let Some(mut img) = images.get_mut(&screen.image) {
        if let Some(data) = img.data.as_mut() {
            data.copy_from_slice(&screen.rgba);
        } else {
            img.data = Some(screen.rgba.clone());
        }
    }
}

fn set_cursor_capture(
    cursor: &mut Query<&mut CursorOptions, With<Window>>,
    captured: &mut MouseCaptured,
    capture: bool,
) {
    captured.0 = capture;
    if let Ok(mut c) = cursor.single_mut() {
        if capture {
            c.grab_mode = CursorGrabMode::Locked;
            c.visible = false;
        } else {
            c.grab_mode = CursorGrabMode::None;
            c.visible = true;
        }
    }
}

/// `Escape` toggles the level-select overlay. While it is open the game is
/// paused; pick an episode and floor with the arrow keys (or WASD), then start
/// it with `Enter`/`Space`. Number keys `1`-`6` jump to an episode directly.
fn handle_level_menu(
    keys: Res<ButtonInput<KeyCode>>,
    controls: Res<TouchControls>,
    mut menu: ResMut<LevelMenu>,
    mut world: ResMut<WorldRes>,
    mut input: ResMut<InputRes>,
    data: Res<DataRes>,
    mut browser: ResMut<Browser>,
    mut captured: ResMut<MouseCaptured>,
    mut cursor: Query<&mut CursorOptions, With<Window>>,
) {
    // `F` (or the FILES button) opens the game-folder picker.
    if keys.just_pressed(KeyCode::KeyF) {
        menu.open = false;
        browser.open_default();
        browser.status = "CHOOSE A FOLDER CONTAINING VSWAP.WL6".to_string();
        set_cursor_capture(&mut cursor, &mut captured, false);
        return;
    }

    // Escape (or the on-screen MENU button) toggles the overlay.
    if keys.just_pressed(KeyCode::Escape) || controls.menu_pressed {
        menu.open = !menu.open;
        if menu.open {
            // Start from the level currently being played.
            menu.episode = world.0.episode;
            menu.map = world.0.map_index;
            menu.difficulty = world.0.difficulty.index();
        }
        menu.show_scores = false;
        set_cursor_capture(&mut cursor, &mut captured, !menu.open);
        return;
    }

    // Desktop shortcuts for the single save slot.
    let save_path = crate::config::persist_path(savegame::FILE_NAME);
    if keys.just_pressed(KeyCode::F5) {
        if let Err(e) = savegame::save(&world.0, &save_path) {
            eprintln!("warning: could not save {}: {e}", save_path.display());
        }
    }
    if keys.just_pressed(KeyCode::F9) {
        if let Some(w) = savegame::load(&data.0, &save_path) {
            world.0 = w;
            menu.open = false;
            input.0 = InputState::default();
            set_cursor_capture(&mut cursor, &mut captured, true);
            return;
        }
    }

    if !menu.open {
        return;
    }
    if menu.show_scores {
        if keys.get_just_pressed().next().is_some() || controls.tap.is_some() {
            menu.show_scores = false;
        }
        return;
    }

    let left = keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA);
    let right = keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD);
    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);

    if left {
        menu.map = if menu.map == 0 { EPISODE_MAPS - 1 } else { menu.map - 1 };
    }
    if right {
        menu.map = (menu.map + 1) % EPISODE_MAPS;
    }
    if up {
        menu.episode = if menu.episode == 0 { EPISODES - 1 } else { menu.episode - 1 };
    }
    if down {
        menu.episode = (menu.episode + 1) % EPISODES;
    }
    // `D` cycles the difficulty.
    if keys.just_pressed(KeyCode::KeyD) {
        menu.difficulty = (menu.difficulty + 1) % 4;
    }

    const EPISODE_KEYS: [KeyCode; EPISODES] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ];
    for (e, &key) in EPISODE_KEYS.iter().enumerate() {
        if keys.just_pressed(key) {
            menu.episode = e;
        }
    }

    // Touch: tap a cell to select it, tap START to play, FILES to pick a data
    // folder, or BACK to close.
    let mut touch_start = false;
    if let Some(p) = controls.tap {
        match menu_hit(p.x, p.y) {
            Some(MenuHit::Episode(e)) => menu.episode = e,
            Some(MenuHit::Map(m)) => menu.map = m,
            Some(MenuHit::Difficulty(d)) => menu.difficulty = d,
            Some(MenuHit::Start) => touch_start = true,
            Some(MenuHit::Save) => {
                if let Err(e) = savegame::save(&world.0, &save_path) {
                    eprintln!("warning: could not save {}: {e}", save_path.display());
                }
            }
            Some(MenuHit::Load) => {
                if let Some(w) = savegame::load(&data.0, &save_path) {
                    world.0 = w;
                    menu.open = false;
                    input.0 = InputState::default();
                    set_cursor_capture(&mut cursor, &mut captured, true);
                    return;
                }
            }
            Some(MenuHit::Scores) => menu.show_scores = true,
            Some(MenuHit::Files) => {
                menu.open = false;
                browser.open_default();
                browser.status = "CHOOSE A FOLDER CONTAINING VSWAP.WL6".to_string();
                set_cursor_capture(&mut cursor, &mut captured, false);
                return;
            }
            Some(MenuHit::Back) => {
                menu.open = false;
                set_cursor_capture(&mut cursor, &mut captured, true);
                return;
            }
            None => {}
        }
    }

    if keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::NumpadEnter)
        || keys.just_pressed(KeyCode::Space)
        || touch_start
    {
        let difficulty = Difficulty::from_index(menu.difficulty);
        if let Some(new_world) = World::new(&data.0, menu.episode, menu.map, difficulty) {
            world.0 = new_world;
        }
        menu.open = false;
        // Don't let the confirming keypress leak into the new level (e.g. Space
        // would immediately trigger a "use" action).
        input.0 = InputState::default();
        set_cursor_capture(&mut cursor, &mut captured, true);
    }
}

fn update_world(time: Res<Time>, input: Res<InputRes>, mut world: ResMut<WorldRes>) {
    let dt = time.delta_secs().min(0.05);
    world.0.update(dt, &input.0);
    if input.0.use_pressed {
        world.0.use_action();
    }
}

fn render_world(
    data: Res<DataRes>,
    world: Res<WorldRes>,
    menu: Res<LevelMenu>,
    scores: Res<ScoresRes>,
    controls: Res<TouchControls>,
    mut screen: ResMut<Screen>,
    mut images: ResMut<Assets<Image>>,
) {
    let screen = &mut *screen;
    let world = &world.0;
    let vswap = &data.0.vswap;

    let cam = Camera {
        x: world.player.x,
        y: world.player.y,
        angle: world.player.angle,
    };

    render_walls(&mut screen.fb, vswap, &world.level, cam, &mut screen.zbuf);
    let sprites = collect_sprites(&world.level, &world.actors, world.player.angle);
    render_sprites(&mut screen.fb, vswap, cam, &screen.zbuf, &sprites);
    render_weapon(&mut screen.fb, vswap, world.player.weapon_sprite());
    draw_status_bar(&mut screen.fb, &data.0.vga, &world.hud);
    match world.state {
        PlayState::Died => {
            crate::render::hud::draw_center_text(
                &mut screen.fb,
                &data.0.vga,
                "YOU DIED",
                70,
                4,
            );
        }
        PlayState::LevelComplete => {
            draw_intermission(
                &mut screen.fb,
                &data.0.vga,
                &Intermission {
                    secret_floor: world.map_index == 9,
                    time_secs: world.elapsed,
                    par_secs: world.par_time * 60.0,
                    kill: world.kill_percent(),
                    secret: world.secret_percent(),
                    treasure: world.treasure_percent(),
                    bonus: world.last_bonus,
                },
            );
        }
        PlayState::GameOver => {
            crate::render::hud::draw_center_text(
                &mut screen.fb,
                &data.0.vga,
                "GAME OVER",
                60,
                4,
            );
            crate::render::hud::draw_center_text(
                &mut screen.fb,
                &data.0.vga,
                "PRESS ANY KEY",
                84,
                0x0f,
            );
        }
        PlayState::Playing => {}
    }

    // The level-select overlay covers the frozen world while it is open;
    // otherwise the touch controls are drawn over the 3D view.
    if menu.open {
        if menu.show_scores {
            crate::render::hud::draw_scores(&mut screen.fb, &data.0.vga, &scores.0);
        } else {
            draw_level_select(
                &mut screen.fb,
                &data.0.vga,
                menu.episode,
                menu.map,
                menu.difficulty,
            );
        }
    } else {
        draw_controls(&mut screen.fb, &data.0.vga, &controls);
    }

    // The "Get Psyched!" intro covers the first moments of floor 1.
    if world.intro_timer > 0.0 {
        crate::render::hud::draw_get_psyched(&mut screen.fb, &data.0.vga);
    }

    // Damage flash: a full-screen red tint that decays, mirroring the
    // original's palette flash.
    screen.fb.to_rgba(&mut screen.rgba);
    let strength = ((world.player.damage_flash / 0.25).clamp(0.0, 1.0) * 0.55)
        .max(world.death_tint * 0.75);
    if strength > 0.0 {
        for px in screen.rgba.chunks_exact_mut(4) {
            let r = px[0] as f32;
            let g = px[1] as f32;
            let b = px[2] as f32;
            px[0] = (r + (255.0 - r) * strength) as u8;
            px[1] = (g * (1.0 - strength)) as u8;
            px[2] = (b * (1.0 - strength)) as u8;
        }
    }
    upload_screen(screen, &mut images);
}

fn play_sounds(
    mut commands: Commands,
    world: Res<WorldRes>,
    bank: Res<SoundBank>,
) {
    // Avoid a wall of sound; play at most a few distinct effects per frame.
    let mut played = 0;
    let mut seen = [false; 512];
    for &s in &world.0.sounds {
        if s >= bank.handles.len() || seen[s] || played >= 4 {
            continue;
        }
        seen[s] = true;
        played += 1;
        commands.spawn((
            AudioPlayer::new(bank.handles[s].clone()),
            PlaybackSettings::DESPAWN,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_transitions(
    mut world: ResMut<WorldRes>,
    data: Res<DataRes>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    controls: Res<TouchControls>,
    mut menu: ResMut<LevelMenu>,
    mut scores: ResMut<ScoresRes>,
    mut captured: ResMut<MouseCaptured>,
    mut cursor: Query<&mut CursorOptions, With<Window>>,
    mut input: ResMut<InputRes>,
) {
    let any_key = keys.get_just_pressed().next().is_some()
        || buttons.just_pressed(MouseButton::Left)
        || controls.tap.is_some();
    match world.0.state {
        PlayState::Died => {
            if world.0.transition_timer > 2.5 {
                world.0.after_death(&data.0);
            }
        }
        PlayState::LevelComplete => {
            if world.0.transition_timer > 5.0 || (world.0.transition_timer > 0.6 && any_key) {
                // Finishing the boss or secret floor ends the episode, so it is
                // a run worth recording.
                if world.0.map_index >= 8 {
                    submit_score(&mut scores, &world.0);
                }
                world.0.next_level(&data.0);
            }
        }
        PlayState::GameOver => {
            if world.0.transition_timer > 3.5 {
                submit_score(&mut scores, &world.0);
                // Back to the level-select menu for a fresh run.
                let (episode, map, difficulty) =
                    (world.0.episode, world.0.map_index, world.0.difficulty);
                if let Some(new_world) = World::new(&data.0, episode, map, difficulty) {
                    world.0 = new_world;
                }
                menu.open = true;
                menu.episode = episode;
                menu.map = map;
                menu.difficulty = difficulty.index();
                input.0 = InputState::default();
                set_cursor_capture(&mut cursor, &mut captured, false);
            }
        }
        PlayState::Playing => {}
    }
}

/// Record a finished run in the high-score table and persist it.
fn submit_score(scores: &mut ScoresRes, world: &World) {
    scores
        .0
        .submit(world.player.score, world.episode, world.map_index);
    scores
        .0
        .save(&crate::config::persist_path("wolf3d-bevy.scores"));
}

/// Wrap signed 16-bit mono PCM in a minimal WAV container so Bevy's audio
/// backend can decode it.
fn wav_from_i16(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let mut v = Vec::with_capacity(44 + data_len);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    v.extend_from_slice(&2u16.to_le_bytes()); // block align
    v.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::{
        DataRes, InputRes, LevelMenu, MouseCaptured, WorldRes, handle_level_menu, wav_from_i16,
    };
    use bevy::audio::{AudioSource, Decodable};
    use bevy::prelude::*;
    use std::sync::Arc;
    use crate::game::actor::Difficulty;
    use crate::game::world::World;

    /// Tap a key once: press it and run the schedule, then clear the
    /// just-pressed edge so the next tap registers again.
    fn tap(app: &mut App, key: KeyCode) {
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(key);
        }
        app.update();
    }

    #[test]
    fn level_menu_navigates_and_starts_selected_level() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = crate::data::GameData::load(&dir).unwrap();
        let world = World::new(&data, 0, 0, Difficulty::Normal).unwrap();

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(DataRes(data));
        app.insert_resource(WorldRes(world));
        app.insert_resource(LevelMenu {
            open: true,
            episode: 0,
            map: 0,
            difficulty: Difficulty::Normal.index(),
            show_scores: false,
        });
        app.insert_resource(MouseCaptured(false));
        app.insert_resource(InputRes::default());
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<crate::touch::TouchControls>();
        app.init_resource::<crate::game::browser::Browser>();
        app.add_systems(Update, handle_level_menu);

        // Right, right, down -> episode 2 (index 1), floor 3 (index 2).
        tap(&mut app, KeyCode::ArrowRight);
        tap(&mut app, KeyCode::ArrowRight);
        tap(&mut app, KeyCode::ArrowDown);
        {
            let menu = app.world().resource::<LevelMenu>();
            assert_eq!((menu.episode, menu.map), (1, 2));
        }

        tap(&mut app, KeyCode::Enter);
        {
            let menu = app.world().resource::<LevelMenu>();
            assert!(!menu.open, "menu should close after starting a level");
        }
        let world = app.world().resource::<WorldRes>();
        assert_eq!((world.0.episode, world.0.map_index), (1, 2));
    }

    /// Wrapping in the grid: left from floor 1 goes to the last floor, and up
    /// from episode 1 goes to the last episode.
    #[test]
    fn level_menu_wraps_selection() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = crate::data::GameData::load(&dir).unwrap();
        let world = World::new(&data, 0, 0, Difficulty::Normal).unwrap();

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(DataRes(data));
        app.insert_resource(WorldRes(world));
        app.insert_resource(LevelMenu {
            open: true,
            episode: 0,
            map: 0,
            difficulty: Difficulty::Normal.index(),
            show_scores: false,
        });
        app.insert_resource(MouseCaptured(false));
        app.insert_resource(InputRes::default());
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<crate::touch::TouchControls>();
        app.init_resource::<crate::game::browser::Browser>();
        app.add_systems(Update, handle_level_menu);

        tap(&mut app, KeyCode::ArrowLeft);
        tap(&mut app, KeyCode::ArrowUp);
        let menu = app.world().resource::<LevelMenu>();
        assert_eq!(menu.map, crate::render::hud::EPISODE_MAPS - 1);
        assert_eq!(menu.episode, crate::render::hud::EPISODES - 1);
    }

    /// The menu's difficulty selection is applied to the started level.
    #[test]
    fn level_menu_selects_difficulty() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = crate::data::GameData::load(&dir).unwrap();
        let world = World::new(&data, 0, 0, Difficulty::Normal).unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(DataRes(data));
        app.insert_resource(WorldRes(world));
        app.insert_resource(LevelMenu {
            open: true,
            episode: 0,
            map: 0,
            difficulty: Difficulty::Normal.index(),
            show_scores: false,
        });
        app.insert_resource(MouseCaptured(false));
        app.insert_resource(InputRes::default());
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<crate::touch::TouchControls>();
        app.init_resource::<crate::game::browser::Browser>();
        app.add_systems(Update, handle_level_menu);

        // One `D` from NORMAL lands on HARD.
        tap(&mut app, KeyCode::KeyD);
        tap(&mut app, KeyCode::Enter);
        let world = app.world().resource::<WorldRes>();
        assert_eq!(world.0.difficulty, Difficulty::Hard);
    }

    /// Guard against regressing the `wav` Bevy feature: the game wraps Wolf3D
    /// PCM in a WAV container, and Bevy's default `audio` feature only enables
    /// vorbis. If `wav` is missing, decoding panics with `UnrecognizedFormat`
    /// (this is exactly what broke at startup before).
    #[test]
    fn wrapped_pcm_decodes_as_wav() {
        let samples: Vec<i16> = (0..128).map(|i| (i as i16) * 64).collect();
        let wav = wav_from_i16(&samples, crate::data::audio::SAMPLE_RATE);
        let source = AudioSource {
            bytes: Arc::from(wav.into_boxed_slice()),
        };
        let decoded: Vec<_> = source.decoder().take(8).collect();
        assert_eq!(decoded.len(), 8, "WAV decoder produced no samples");
    }

    /// Decode every non-empty Wolf3D sound chunk through Bevy's WAV path, so a
    /// malformed/unsupported chunk in the data set is caught here rather than
    /// as a mid-game panic.
    #[test]
    fn all_real_sound_chunks_decode() {
        let Some(dir) = crate::data::find_data_dir() else {
            return;
        };
        let data = crate::data::GameData::load(&dir).unwrap();
        for i in 0..data.audio.count() {
            let samples = data.audio.sound_i16(i);
            if samples.is_empty() {
                continue; // The data set has unused empty chunks; nothing plays them.
            }
            let wav = wav_from_i16(&samples, crate::data::audio::SAMPLE_RATE);
            let source = AudioSource {
                bytes: Arc::from(wav.into_boxed_slice()),
            };
            assert!(
                source.decoder().count() > 0,
                "sound chunk {i} failed to decode"
            );
        }
    }
}
