//! The match simulation: a port of the original 68000 match code.
//!
//! Nothing in here draws or touches the window; [`state::Match`] is driven by
//! [`state::Match::step`] and read by `render`.

pub mod actions;
pub mod ai;
pub mod arena;
pub mod career;
pub mod cup;
pub mod entity;
pub mod events;
pub mod geometry;
pub mod items;
pub mod league;
pub mod m68k;
pub mod medic;
pub mod player;
pub mod replay;
pub mod rng;
pub mod roster;
pub mod scene;
pub mod setup;
pub mod squad;
pub mod state;
pub mod statistics;
pub mod text;
pub mod token;

pub use setup::MatchSetup;
pub use state::Match;
