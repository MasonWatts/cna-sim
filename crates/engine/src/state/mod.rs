//! Tier 2: mutable game state — the serializable "save-game" (design.md §3.1).
//!
//! Everything a physical player erases and rewrites: the map's placed objects, unit
//! strengths/positions/status, and whose turn/phase it is. Grows to include units and
//! the sequence-of-play state machine.

pub mod air;
pub mod combat;
pub mod command;
pub mod game;
pub mod map;
pub mod naval;
pub mod scenario;
pub mod unit;
pub mod view;
pub mod zoc;
