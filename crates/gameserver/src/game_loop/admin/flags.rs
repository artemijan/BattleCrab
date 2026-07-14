//! GM flag toggles — `AdminInvul`/`AdminUndying`/`AdminHide`. Each flips a bit
//! in [`AdminFlags`](crate::model::components::AdminFlags) on the GM or the
//! targeted player.

use crate::model::components::AdminFlags;
use crate::model::Player;
use crate::network::server_packets;
use crate::world::World;

use super::{current_target, send_message};

/// The GM flags togglable via `AdminFlags`.
#[derive(Clone, Copy)]
pub(super) enum GmFlag {
    Invul,
    Undying,
    Hidden,
}

impl GmFlag {
    fn label(self) -> &'static str {
        match self {
            GmFlag::Invul => "Invulnerability",
            GmFlag::Undying => "Undying",
            GmFlag::Hidden => "Hide",
        }
    }
}

/// Flip a GM flag on `target`, returning its new state.
fn set_flag(world: &mut World, target: i32, flag: GmFlag) -> bool {
    let mut flags = world.objects.get_component::<AdminFlags>(&target).copied().unwrap_or_default();
    let now = match flag {
        GmFlag::Invul => {
            flags.invul = !flags.invul;
            flags.invul
        }
        GmFlag::Undying => {
            flags.undying = !flags.undying;
            flags.undying
        }
        GmFlag::Hidden => {
            flags.hidden = !flags.hidden;
            flags.hidden
        }
    };
    world.objects.add_components(&target, flags);
    now
}

/// `AdminHide`'s `//hide` — toggle the GM's visibility to other players (Java
/// `setInvisible` + `decayMe`/`spawnMe`). Hiding sends `DeleteObject` to nearby
/// players; unhiding re-runs the visibility exchange. While hidden, the
/// visibility system won't describe the GM to anyone (see `send_char_info`).
pub(super) fn admin_hide(world: &mut World, client_id: u32, object_id: i32) {
    let hidden = set_flag(world, object_id, GmFlag::Hidden);
    if hidden {
        super::helpers::broadcast_to_others(world, object_id, &server_packets::delete_object(object_id));
        send_message(world, client_id, "You are now hidden.");
    } else {
        super::visibility::on_enter_world(world, client_id, object_id);
        send_message(world, client_id, "You are now visible.");
    }
}

/// `//invul` / `//undying` — toggle the flag on the GM.
pub(super) fn toggle_flag(world: &mut World, client_id: u32, object_id: i32, flag: GmFlag) {
    let on = set_flag(world, object_id, flag);
    send_message(world, client_id, &format!("{} {}.", flag.label(), if on { "enabled" } else { "disabled" }));
}

/// `//setinvul` / `//setundying` — toggle the flag on the targeted player.
pub(super) fn toggle_flag_on_target(world: &mut World, client_id: u32, object_id: i32, flag: GmFlag) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        send_message(world, client_id, "Select a player first.");
        return;
    };
    let on = set_flag(world, target, flag);
    send_message(world, client_id, &format!("Target {} {}.", flag.label().to_lowercase(), if on { "enabled" } else { "disabled" }));
}
