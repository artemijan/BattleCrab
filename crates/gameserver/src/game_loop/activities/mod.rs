//! Optional side content — the systems a player opts into for their own sake
//! rather than to advance a character: fishing, the Monster Race Track, the
//! weekly lottery and the Four Sepulchers time attack.
//!
//! They share no code, only a shape: each is a self-contained engine with its
//! own wall-clock schedule, its own NPC surface and its own reward table.

pub(crate) mod fishing;
pub(crate) mod four_sepulchers;
pub(crate) mod lottery;
pub(crate) mod monster_race;
