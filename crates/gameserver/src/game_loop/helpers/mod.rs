//! Shared helpers for the packet handlers, split by theme: lookups, sends,
//! broadcasts, vitals, inventory and position.

use crate::game_loop::guard::maybe_position;
use crate::model;
use crate::model::Player;
use crate::model::components::{Movement, RegionCell, StatModifiers, Vitals};
use crate::model::inventory::Inventory;
use crate::model::npc::Npc;
use crate::model::stats::Stat;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

mod broadcast;
mod inventory;
mod lookup;
mod position;
mod send;
mod vitals;

pub(crate) use broadcast::*;
pub(crate) use inventory::*;
pub(crate) use lookup::*;
pub(crate) use position::*;
pub(crate) use send::*;
pub(crate) use vitals::*;

// Moved to their owning modules; re-exported so existing helpers:: call
// sites keep working.
pub(crate) use crate::game_loop::combat::run_queued_action;
pub(crate) use crate::game_loop::items::block_inventory;
pub(crate) use crate::game_loop::npc::ai::{
    force_attack_target, set_active_intention, set_attack_intention, set_move_to_intention,
};
pub(crate) use crate::game_loop::npc::say::{npc_say, npc_say_param, npc_say_text};
pub(crate) use crate::game_loop::visibility::visible_creatures;

/// Re-exported from [`crate::scheduler`], which owns the tick and sits below
/// both `game_loop` and `config`. Kept visible here because `helpers` is where
/// every game-loop caller already looks for it.
pub(crate) use crate::scheduler::ms_to_ticks;

#[cfg(test)]
mod tests {
    use crate::game_loop::helpers::format_amount;

    #[test]
    fn formats_thousands() {
        assert_eq!(format_amount(0), "0");
        assert_eq!(format_amount(999), "999");
        assert_eq!(format_amount(1_000), "1,000");
        assert_eq!(format_amount(200_000), "200,000");
        assert_eq!(format_amount(1_234_567), "1,234,567");
        assert_eq!(format_amount(-4_200), "-4,200");
    }
}
