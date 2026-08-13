//! Gear equip/unequip handlers (`UseItem`, `RequestUnEquipItem`) and the
//! `EtcItem` "use" dispatch (`ExtractableItems` for pack/box items).

use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::skill_by_id;
use tracing::warn;

use crate::data::item_data::ItemHandler;
use crate::game_loop::helpers::item_id_of;
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

mod equip;
mod etc_item;
mod handlers;
mod inventory;
mod shots;

pub(crate) use equip::*;
pub(crate) use etc_item::*;
pub(crate) use handlers::*;
pub(crate) use inventory::*;
pub(crate) use shots::*;
