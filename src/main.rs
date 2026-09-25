//! Desktop entry point.

use bevy::app::AppExit;

fn main() {
    // The app opens the folder picker instead of exiting, but still propagate
    // a nonzero exit code if `run` fails outright.
    if let AppExit::Error(code) = wolf3d_bevy::app::run() {
        std::process::exit(code.get() as i32);
    }
}
