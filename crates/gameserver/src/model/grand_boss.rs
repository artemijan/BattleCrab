//! Grand bosses — the `grandboss_data` slice Java's `GrandBossManager` keeps in
//! memory (`_storedInfo` StatSet + `_bossStatus`): each boss's stored spawn
//! location, HP/MP, respawn time and status, loaded once at boot. The spawn /
//! AI / boss-zone lifecycle that drives `status` over time is a later milestone
//! (G21); this slice backs the read-only `//grandboss` admin panel.

/// One `grandboss_data` row (Java `_storedInfo` StatSet + the `_bossStatus`
/// entry). Keyed by `boss_id` in [`crate::world::World::grand_bosses`].
#[derive(Debug, Clone)]
pub struct GrandBoss {
    pub boss_id: i32,
    pub loc_x: i32,
    pub loc_y: i32,
    pub loc_z: i32,
    pub heading: i32,
    /// Java `respawn_time` (epoch millis); `0` for a boss that is currently up.
    pub respawn_time: i64,
    pub current_hp: f64,
    pub current_mp: f64,
    /// Java `_bossStatus`. For Antharas/Valakas/Baium: 0 alive, 1 waiting,
    /// 2 in-fight, 3 dead. For the others (Queen Ant/Orfen/Core): 0 alive,
    /// 1 dead. Driven by the (unported, G21) boss AI; static from the DB here.
    pub status: i32,
}
