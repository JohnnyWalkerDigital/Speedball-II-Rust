//! Speedball 2 match engine, ported from a reverse-engineered C# version.
//!
//! Layout:
//! - `data`: reading the original disk images and their lookup tables
//! - `game`: the match simulation (no I/O, no drawing)
//! - `graphics`: decoding bitplane graphics into images
//! - `render`: drawing a match onto a framebuffer
//! - `sound`: the original sound driver and an Amiga sound-chip emulation
//! - `app`: window, keyboard and timing

mod app;
mod config;
mod data;
mod game;
mod graphics;
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
    let result = match &config.export_sounds {
        Some(dir) => app::export_sounds(&config, dir),
        None => app::run(config),
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
