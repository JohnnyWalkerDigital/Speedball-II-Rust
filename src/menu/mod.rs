//! The front end: title screens, menus and the game modes around matches.
//!
//! Ported from the Amiga version's menu code. It reads its strings, fonts
//! and pictures from Amiga memory and the Amiga disk, so it needs the Amiga
//! files whichever graphics the match uses.

pub mod assets;
pub mod cup;
pub mod flow;
pub mod host;
pub mod league;
pub mod manager;
pub mod panels;
pub mod play;
pub mod ram;
pub mod saving;
pub mod text;
pub mod ui;
