//! Shared helpers for the packet handlers, split by theme: lookups, sends,
//! broadcasts, vitals, inventory and position.
//!
//! The `pub(crate) use` lines below re-export this module's **own** submodules,
//! which is the whole point of the split. Nothing else belongs here: a symbol
//! that lives in another module is imported from that module by every caller,
//! never forwarded through this one. A convenience re-export would make
//! `helpers` look like the home of something it does not own — `ms_to_ticks`
//! belongs to `crate::scheduler`, `npc_say` to `npc::say`, and so on.

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
