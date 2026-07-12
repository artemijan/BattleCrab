//! The compiled-in scripts — the Rust counterpart of
//! `dist/game/data/scripts/**.java` (which Java compiles at boot; here each
//! script is a module registering a `QuestScript` trait object in
//! [`build_registry`]). Framework in `game_loop/quests.rs`; this module is
//! only the content.

pub mod clan_master;
pub mod q00258_bring_wolf_pelts;
pub mod q00320_bones_tell_the_future;

use std::sync::Arc;

use crate::game_loop::quests::{QuestRegistry, QuestScript};

/// Java's `ScriptEngineManager.executeScriptList()` + the `Quest`
/// constructor self-registration, collapsed into one boot-time list.
pub fn build_registry() -> QuestRegistry {
    let scripts: Vec<Arc<dyn QuestScript>> = vec![
        Arc::new(q00258_bring_wolf_pelts::Q00258BringWolfPelts),
        Arc::new(q00320_bones_tell_the_future::Q00320BonesTellTheFuture),
        Arc::new(clan_master::ClanMaster),
    ];
    QuestRegistry::new(scripts)
}
