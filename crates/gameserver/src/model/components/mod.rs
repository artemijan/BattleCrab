//! ECS components shared by players and NPCs — stage 2 of the `bevy_ecs`
//! adoption (`PLAN_ECS_STAGE2.md` §2). Data only: components are split
//! along *system access seams* (what a per-tick sweep reads/writes without
//! the rest of the object), not per field, and carry no game logic beyond
//! trivial accessors. Player-only / NPC-only state stays in the (shrinking)
//! fat structs in `model/mod.rs` / `model/npc.rs` until its own phase.

pub mod combat;
pub mod commerce;
pub mod player;
pub mod skills;
pub mod social;
pub mod space;
pub mod stats;
pub mod summons;
