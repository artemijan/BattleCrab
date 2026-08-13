//! Raid and grand bosses: the per-boss encounter scripts plus the shared
//! framework they sit on (persistent status in [`grand_boss`], DB-backed
//! respawns in [`boss_respawn`], the Java top-3 threat table in
//! [`boss_threat`], the level-gap curse in [`raid_curse`], and small lair
//! helpers in [`common`]).
//!
//! Submodules are re-exported from [`super`] (`game_loop`), so callers keep
//! addressing them as `game_loop::antharas`, `game_loop::baium`, etc.

pub(crate) mod antharas;
pub(crate) mod baium;
pub(crate) mod boss_respawn;
pub(crate) mod boss_threat;
pub(crate) mod common;
pub(crate) mod core_boss;
pub(crate) mod dr_chaos;
pub(crate) mod frintezza;
pub(crate) mod grand_boss;
pub(crate) mod orfen;
pub(crate) mod queen_ant;
pub(crate) mod raid_curse;
pub(crate) mod sailren;
pub(crate) mod valakas;
