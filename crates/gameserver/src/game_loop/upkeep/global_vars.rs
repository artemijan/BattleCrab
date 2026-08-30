//! Java `GlobalVariablesManager` — server-wide state that must outlive a
//! restart, keyed by a plain string.
//!
//! The `global_variables` table and its sea-orm entity have existed in this
//! port since the schema baseline; nothing read or wrote them. Two markers
//! ("no GlobalVariables table in the port") were wrong about that.
//!
//! Writes go through immediately rather than on Java's 30-minute `onSave`
//! timer. That matches how the rest of this port persists small global state,
//! and closes the window where a crash loses the last half hour — the values
//! stored here are precisely the ones a restart is supposed to preserve.

use crate::db::DbCommand;
use crate::world::World;

/// Java `GlobalVariablesManager.DAILY_TASK_RESET`.
pub(crate) const DAILY_TASK_RESET: &str = "DAILY_TASK_RESET";

/// The per-hall Four Sepulchers entry stamp, keyed as Java composes it:
/// `"FourSepulchers" + npcId`.
pub(crate) fn four_sepulchers_key(manager_npc_id: i32) -> String {
    format!("FourSepulchers{manager_npc_id}")
}

/// `getLong(name, default)`.
pub(crate) fn get_i64(world: &World, name: &str, default: i64) -> i64 {
    world
        .global_vars
        .get(name)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default)
}

/// `getBoolean(name, default)`. Java stores these through `String.valueOf`, so
/// the stored text is `true`/`false`.
pub(crate) fn get_bool(world: &World, name: &str, default: bool) -> bool {
    world
        .global_vars
        .get(name)
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

/// `set(name, value)` — update the in-memory map and persist.
pub(crate) fn set(world: &mut World, name: &str, value: impl ToString) {
    let value = value.to_string();
    world.global_vars.insert(name.to_string(), value.clone());
    let _ = world.db.send(DbCommand::SaveGlobalVariable {
        var: name.to_string(),
        value,
    });
}
