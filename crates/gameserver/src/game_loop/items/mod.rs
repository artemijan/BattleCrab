//! Gear equip/unequip handlers (`UseItem`, `RequestUnEquipItem`) and the
//! `EtcItem` "use" dispatch (`ExtractableItems` for pack/box items).

pub(crate) mod armor_sets;
pub(crate) mod augment;
mod conditions;
pub(crate) mod cursed_weapon;
pub mod enchant;
mod equip;
mod etc_item;
pub(crate) mod ground_items;
mod handlers;
pub(crate) mod item_auction;
pub(crate) mod item_mana;
mod shots;

pub(crate) use conditions::{check_condition, check_item_restriction, is_condition_attached};

use equip::cursed_weapon_blocks_equip;

pub(crate) use equip::{
    destroy_item_by_id, destroy_item_by_object_id, finish_equip_change,
    finish_equipped_item_destroyed, refresh_after_paperdoll_change, refresh_equip_state,
    unequip_if_worn, unequipped_by_removal, use_equipable_item,
};
use etc_item::use_etc_item;

pub(crate) use etc_item::use_item_by_object_id;

pub(crate) use handlers::{
    handle_request_crystallize_item, handle_request_destroy_item, handle_request_item_list,
    handle_request_save_inventory_order, handle_request_un_equip_item, handle_use_item,
    send_item_message,
};
pub(crate) use shots::{
    auto_shots, charge_fish_shot, charge_shot, handle_request_auto_soul_shot, recharge_shots,
    remove_auto_shot,
};
