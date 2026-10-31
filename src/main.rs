//! Speedball 2 match engine.
//!
//! Layout:
//! - `data`: reading the original disk images and their lookup tables
//! - `game`: the match simulation (no I/O, no drawing)
//! - `content`: where the graphics come from: the originals or an asset pack
//! - `graphics`: decoding bitplane graphics into images
//! - `render`: drawing a match onto a framebuffer
//! - `sound`: the original sound driver and an Amiga sound-chip emulation
//! - `app`: window, keyboard and timing

mod app;
mod config;
mod content;
mod data;
#[cfg(test)]
mod front_end_tests;
mod game;
mod graphics;
mod menu;
mod render;
#[cfg(test)]
mod simulation_tests;
mod sound;

use std::process::ExitCode;

use config::{Config, USAGE};

fn main() -> ExitCode {
    let config = match Config::from_args(std::env::args().skip(1)) {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let result = match (&config.export_sounds, &config.extract) {
        (Some(dir), _) => app::export_sounds(&config, dir),
        (None, Some(dir)) => app::extract(&config, dir),
        (None, None) => app::run(config),
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
