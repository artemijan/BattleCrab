//! GM flag toggles — `AdminInvul`/`AdminUndying`/`AdminHide`. Each flips a bit
//! in [`AdminFlags`](crate::model::components::AdminFlags) on the GM or the
//! targeted player.

use crate::model::Player;
use crate::model::components::AdminFlags;
use crate::network::server_packets;
use crate::world::World;

use super::{current_target, send_message, send_sm};

/// The GM flags togglable via `AdminFlags`.
#[derive(Clone, Copy)]
pub(super) enum GmFlag {
    Invul,
    Undying,
}

impl GmFlag {
    fn label(self) -> &'static str {
        match self {
            GmFlag::Invul => "Invulnerability",
            GmFlag::Undying => "Undying",
        }
    }
}

/// Flip a GM flag on `target`, returning its new state.
fn set_flag(world: &mut World, target: i32, flag: GmFlag) -> bool {
    let mut flags = world
        .objects
        .get_component::<AdminFlags>(&target)
        .copied()
        .unwrap_or_default();
    let now = match flag {
        GmFlag::Invul => {
            flags.invul = !flags.invul;
            flags.invul
        }
        GmFlag::Undying => {
            flags.undying = !flags.undying;
            flags.undying
        }
    };
    world.objects.add_components(&target, flags);
    now
}

/// Send `ExUserInfoAbnormalVisualEffect` to the GM's own client so the
/// invisible state is actually rendered on the character (the STEALTH glow),
/// or cleared when visible again (Java sends it on every invis toggle).
fn send_invisible_visual(world: &World, client_id: u32, object_id: i32, invisible: bool) {
    // Keep the current transform id in the packet so toggling invisibility on a
    // transformed GM doesn't drop the transform model.
    let transform = world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(0, |p| p.transform_display_id);
    // Carry the buff-driven visuals through too, so toggling invisibility on a
    // stunned/poisoned GM doesn't wipe those from their own view.
    let visuals = crate::game_loop::abnormal::visual_effects(world, object_id);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(
            crate::network::user_info::ex_user_info_abnormal_visual_effect(
                object_id, invisible, transform, &visuals,
            ),
        );
    }
}

/// `AdminHide`'s `//hide` — toggle the GM's visibility to other players (Java
/// `setInvisible` + `decayMe`/`spawnMe`). Hiding sends `DeleteObject` to nearby
/// players; unhiding re-runs the visibility exchange. While hidden, the
/// visibility system won't describe the GM to anyone (see `send_char_info`).
/// Either way the GM's own client gets the STEALTH abnormal-visual update so
/// the invisible state shows on the character.
pub(super) fn admin_hide(world: &mut World, client_id: u32, object_id: i32) {
    let hidden = !world
        .objects
        .get_component::<AdminFlags>(&object_id)
        .is_some_and(|f| f.hidden);
    set_hidden(world, client_id, object_id, hidden);
}

/// `AdminEffects`' `admin_invis`/`admin_invisible` (`hidden = true`) and
/// `admin_vis`/`admin_visible` (`hidden = false`) — Java *sets* the state
/// rather than toggling, so `//vis` while visible must stay visible (the old
/// toggle alias here hid you instead).
pub(super) fn admin_set_hidden(world: &mut World, client_id: u32, object_id: i32, hidden: bool) {
    set_hidden(world, client_id, object_id, hidden);
}

/// The gm_menu "Invis" button (`admin_invis_menu`): Java toggles on the
/// current state and re-serves `gm_menu.htm` so the panel stays up
/// (`AdminEffects` → `AdminHtml.showAdminHtml("gm_menu.htm")`).
pub(super) fn admin_invis_menu(world: &mut World, client_id: u32, object_id: i32) {
    admin_hide(world, client_id, object_id);
    super::menu::show_admin_html(world, client_id, "gm_menu.htm");
}

/// `admin_setinvis` — toggle invisibility on the *targeted player* (Java
/// flips any targeted Creature; NPC invisibility isn't modeled in the port,
/// so non-player targets are refused like Java's null-target branch).
pub(super) fn admin_setinvis(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        send_message(world, client_id, "Invalid target.");
        return;
    };
    let hidden = !world
        .objects
        .get_component::<AdminFlags>(&target)
        .is_some_and(|f| f.hidden);
    let target_client = super::helpers::client_for_player(world, target).unwrap_or(0);
    set_hidden(world, target_client, target, hidden);
}

/// Java `WorldObject.setInvisible` + the `admin_invis`/`admin_vis` packet
/// tail: on hide, every player targeting the GM drops the selection (with
/// `TargetUnselected`, same order as the visibility system's walk-away path)
/// before the `DeleteObject` lands; on unhide the visibility exchange
/// re-describes the GM to everyone (`broadcastInfo`). Either way the owner's
/// client gets the STEALTH abnormal-visual update. While hidden,
/// `visibility::send_char_info` and `party::broadcast_user_info` suppress
/// the CharInfo, and NPC aggro skips the GM (`notices_target`). Java also
/// aborts observers' in-flight attacks/casts and idles NPC AI on hide; the
/// port's mobs re-validate through `notices_target` instead.
fn set_hidden(world: &mut World, client_id: u32, object_id: i32, hidden: bool) {
    let mut flags = world
        .objects
        .get_component::<AdminFlags>(&object_id)
        .copied()
        .unwrap_or_default();
    flags.hidden = hidden;
    world.objects.add_components(&object_id, flags);
    if hidden {
        // Everyone with the GM selected loses the selection first.
        let mut watchers: Vec<i32> = Vec::new();
        world
            .objects
            .for_each_mut::<(&Player, &crate::model::components::TargetRef)>(|(p, t)| {
                if t.0 == Some(object_id) && p.object_id != object_id {
                    watchers.push(p.object_id);
                }
            });
        for watcher in watchers {
            crate::game_loop::target::drop_target_notify(world, watcher);
        }
        super::helpers::broadcast_to_others(
            world,
            object_id,
            &server_packets::delete_object(object_id),
        );
        send_message(world, client_id, "Now, you cannot be seen.");
    } else {
        super::visibility::on_enter_world(world, client_id, object_id);
        send_message(world, client_id, "Now, you can be seen.");
    }
    send_invisible_visual(world, client_id, object_id, hidden);
}

/// `AdminAdmin`'s `//silence` — toggle message-refusal (chat block) mode on the
/// GM. Java `setSilenceMode` + the MESSAGE_REFUSAL/ACCEPTANCE_MODE system
/// message; the whisper handler refuses PMs to a silenced player.
pub(super) fn admin_silence(world: &mut World, client_id: u32, object_id: i32) {
    let mut flags = world
        .objects
        .get_component::<AdminFlags>(&object_id)
        .copied()
        .unwrap_or_default();
    flags.silence = !flags.silence;
    let on = flags.silence;
    world.objects.add_components(&object_id, flags);
    send_sm(
        world,
        client_id,
        if on {
            server_packets::sm_ids::MESSAGE_REFUSAL_MODE
        } else {
            server_packets::sm_ids::MESSAGE_ACCEPTANCE_MODE
        },
    );
    // Java `setSilenceMode` → `EtcStatusUpdate`: redraw the chat-block icon.
    super::helpers::send_etc_status_update(world, client_id, object_id);
    // Java re-shows the GM menu after toggling.
    super::menu::show_admin_html(world, client_id, "gm_menu.htm");
}

/// `//invul` / `//undying` — toggle the flag on the GM.
pub(super) fn toggle_flag(world: &mut World, client_id: u32, object_id: i32, flag: GmFlag) {
    let on = set_flag(world, object_id, flag);
    send_message(
        world,
        client_id,
        &format!(
            "{} {}.",
            flag.label(),
            if on { "enabled" } else { "disabled" }
        ),
    );
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
    let Some(access_level) = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.access_level)
    else {
        return;
    };
    let mut flags = world
        .objects
        .get_component::<AdminFlags>(&object_id)
        .copied()
        .unwrap_or_default();

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
pub(super) fn toggle_flag_on_target(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    flag: GmFlag,
) {
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        send_message(world, client_id, "Select a player first.");
        return;
    };
    let on = set_flag(world, target, flag);
    send_message(
        world,
        client_id,
        &format!(
            "Target {} {}.",
            flag.label().to_lowercase(),
            if on { "enabled" } else { "disabled" }
        ),
    );
}

/// `AdminEffects`' `//ave_abnormal <NAME> [radius]` — **toggle** an abnormal
/// visual effect on the current target (or self when untargeted), or on
/// everyone within `radius`. Java's `performAbnormalVisualEffect` starts the
/// effect when absent and stops it when present, which is what makes the
/// command a toggle.
///
/// Now that the visual runtime exists (G19), this is just a pinned entry in
/// `AdminVisuals` folded alongside the buff-derived ones.
pub(super) fn admin_ave_abnormal(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    use crate::model::components::AdminVisuals;

    let Some(&name) = args.first() else {
        send_message(
            world,
            client_id,
            "Usage: //ave_abnormal <EFFECT_NAME> [radius]",
        );
        return;
    };
    let Some(ave) = crate::model::skill::abnormal_visual_client_id(&name.to_uppercase()) else {
        send_message(
            world,
            client_id,
            &format!("Unknown abnormal visual effect: {name}"),
        );
        return;
    };
    let radius = args.get(1).and_then(|r| r.parse::<i32>().ok()).unwrap_or(0);

    // Target set: everyone in radius, else the target, else self.
    let targets: Vec<i32> = if radius > 0 {
        let Some(origin) = world
            .objects
            .get_component::<crate::model::components::Position>(&object_id)
            .copied()
        else {
            return;
        };
        let mut out = Vec::new();
        for cs in world.clients.values() {
            if let crate::session::ClientSession::InGame(s) = cs {
                let oid = s.player_object_id();
                if world
                    .objects
                    .get_component::<crate::model::components::Position>(&oid)
                    .is_some_and(|p| {
                        let (dx, dy) = ((p.x - origin.x) as f64, (p.y - origin.y) as f64);
                        (dx * dx + dy * dy).sqrt() <= radius as f64
                    })
                {
                    out.push(oid);
                }
            }
        }
        out
    } else {
        vec![super::current_target(world, object_id).unwrap_or(object_id)]
    };

    let mut toggled_on = 0;
    for target in &targets {
        let now_on = {
            let entry = world.objects.get_component_mut::<AdminVisuals>(target);
            match entry {
                Some(v) => {
                    if let Some(pos) = v.0.iter().position(|&x| x == ave) {
                        v.0.remove(pos);
                        false
                    } else {
                        v.0.push(ave);
                        true
                    }
                }
                None => {
                    world
                        .objects
                        .add_components(target, AdminVisuals(vec![ave]));
                    true
                }
            }
        };
        toggled_on += i32::from(now_on);
        // Re-broadcast so the change is visible immediately, to the owner and
        // to everyone who can see them.
        crate::game_loop::party::broadcast_user_info(world, *target);
        if let Some(cid) = crate::game_loop::helpers::client_for_player(world, *target) {
            let visuals = crate::game_loop::abnormal::visual_effects(world, *target);
            let transform = world
                .objects
                .get_component::<Player>(target)
                .map_or(0, |p| p.transform_display_id);
            let hidden = world
                .objects
                .get_component::<crate::model::components::AdminFlags>(target)
                .is_some_and(|f| f.hidden);
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(
                    crate::network::user_info::ex_user_info_abnormal_visual_effect(
                        *target, hidden, transform, &visuals,
                    ),
                );
            }
        }
    }
    send_message(
        world,
        client_id,
        &format!(
            "{name}: enabled on {toggled_on}, disabled on {} of {} target(s).",
            targets.len() as i32 - toggled_on,
            targets.len()
        ),
    );
}
