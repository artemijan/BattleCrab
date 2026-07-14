//! GM flag toggles — `AdminInvul`/`AdminUndying`/`AdminHide`. Each flips a bit
//! in [`AdminFlags`](crate::model::components::AdminFlags) on the GM or the
//! targeted player.

use crate::model::components::AdminFlags;
use crate::model::Player;
use crate::network::server_packets;
use crate::world::World;

use super::{current_target, send_message, send_sm};

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

/// Send `ExUserInfoAbnormalVisualEffect` to the GM's own client so the
/// invisible state is actually rendered on the character (the STEALTH glow),
/// or cleared when visible again (Java sends it on every invis toggle).
fn send_invisible_visual(world: &World, client_id: u32, object_id: i32, invisible: bool) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::user_info::ex_user_info_abnormal_visual_effect(object_id, invisible));
    }
}

/// `AdminHide`'s `//hide` — toggle the GM's visibility to other players (Java
/// `setInvisible` + `decayMe`/`spawnMe`). Hiding sends `DeleteObject` to nearby
/// players; unhiding re-runs the visibility exchange. While hidden, the
/// visibility system won't describe the GM to anyone (see `send_char_info`).
/// Either way the GM's own client gets the STEALTH abnormal-visual update so
/// the invisible state shows on the character.
pub(super) fn admin_hide(world: &mut World, client_id: u32, object_id: i32) {
    let hidden = set_flag(world, object_id, GmFlag::Hidden);
    if hidden {
        super::helpers::broadcast_to_others(world, object_id, &server_packets::delete_object(object_id));
        send_message(world, client_id, "You are now hidden.");
    } else {
        super::visibility::on_enter_world(world, client_id, object_id);
        send_message(world, client_id, "You are now visible.");
    }
    send_invisible_visual(world, client_id, object_id, hidden);
}

/// `AdminAdmin`'s `//silence` — toggle message-refusal (chat block) mode on the
/// GM. Java `setSilenceMode` + the MESSAGE_REFUSAL/ACCEPTANCE_MODE system
/// message; the whisper handler refuses PMs to a silenced player.
pub(super) fn admin_silence(world: &mut World, client_id: u32, object_id: i32) {
    let mut flags = world.objects.get_component::<AdminFlags>(&object_id).copied().unwrap_or_default();
    flags.silence = !flags.silence;
    let on = flags.silence;
    world.objects.add_components(&object_id, flags);
    send_sm(
        world,
        client_id,
        if on { server_packets::sm_ids::MESSAGE_REFUSAL_MODE } else { server_packets::sm_ids::MESSAGE_ACCEPTANCE_MODE },
    );
    // Java `setSilenceMode` → `EtcStatusUpdate`: redraw the chat-block icon.
    super::helpers::send_etc_status_update(world, client_id, object_id);
    // Java re-shows the GM menu after toggling.
    super::menu::show_admin_html(world, client_id, "gm_menu.htm");
}

/// `//invul` / `//undying` — toggle the flag on the GM.
pub(super) fn toggle_flag(world: &mut World, client_id: u32, object_id: i32, flag: GmFlag) {
    let on = set_flag(world, object_id, flag);
    send_message(world, client_id, &format!("{} {}.", flag.label(), if on { "enabled" } else { "disabled" }));
}

/// Java `EnterWorld.runImpl`'s GM startup block (the `gmStartupProcess:`
/// label): apply the configured default GM state on login, each gated by the
/// matching `admin_*` access right — exactly as Java gates `admin_hide`,
/// `admin_invul`, `admin_invisible`, `admin_silence`, `admin_diet`. Runs once,
/// just before the enter-world visibility broadcast, so an invisible GM is
/// never described to nearby players in the first place (no `DeleteObject`
/// needed, unlike the runtime `//hide`).
///
/// The caller has already confirmed the player is a GM.
pub(crate) fn apply_gm_startup(world: &mut World, client_id: u32, object_id: i32) {
    let gm = world.data.gm;
    let Some(access_level) = world.objects.get_component::<Player>(&object_id).map(|p| p.access_level)
    else {
        return;
    };
    let mut flags = world.objects.get_component::<AdminFlags>(&object_id).copied().unwrap_or_default();

    // `GMStartupBuilderHide`: hide + the three retail "…is default for builder"
    // notices, then **break** — Java deliberately skips the rest so the custom
    // L2J invul/invis/silence/diet flags aren't stacked on the builder default.
    if gm.startup_builder_hide && world.data.admin.has_access("admin_hide", access_level) {
        flags.hidden = true;
        world.objects.add_components(&object_id, flags);
        send_message(world, client_id, "hide is default for builder.");
        send_message(world, client_id, "FriendAddOff is default for builder.");
        send_message(world, client_id, "whisperoff is default for builder.");
        send_invisible_visual(world, client_id, object_id, true);
        register_gm(world, object_id, access_level);
        return;
    }

    if gm.startup_invulnerable && world.data.admin.has_access("admin_invul", access_level) {
        flags.invul = true;
    }
    if gm.startup_invisible && world.data.admin.has_access("admin_invisible", access_level) {
        flags.hidden = true;
    }
    if gm.startup_silence && world.data.admin.has_access("admin_silence", access_level) {
        flags.silence = true;
    }
    if gm.startup_diet_mode && world.data.admin.has_access("admin_diet", access_level) {
        flags.diet = true;
    }
    world.objects.add_components(&object_id, flags);
    if flags.hidden {
        send_invisible_visual(world, client_id, object_id, true);
    }
    if flags.silence {
        // Redraw the chat-block icon for a GM that logs in silenced.
        super::helpers::send_etc_status_update(world, client_id, object_id);
    }
    register_gm(world, object_id, access_level);
}

/// Java `AdminData.addGm(player, hidden)`: register the live GM with a
/// hidden-from-`//gmlist` flag of
/// `!GMStartupAutoList || !hasAccess("admin_gmliston")`. There is no `//gmlist`
/// consumer yet (GMs are derived on demand from `is_gm` — see
/// `admin::moderation::admin_gmchat`), so nothing tracks the flag today.
/// TODO(G14): keep the hidden flag once `//gmlist`/`//gmliston` land.
fn register_gm(world: &World, _object_id: i32, access_level: i32) {
    let _hidden = !world.data.gm.startup_auto_list
        || !world.data.admin.has_access("admin_gmliston", access_level);
    // No-op until the GM list exists.
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
