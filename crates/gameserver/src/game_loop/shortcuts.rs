//! Shortcut-panel and macro handlers (G9.6, plan:
//! `docs/PLAN_MACROS_SHORTCUTS.md`) — ports of `RequestShortCutReg`/`Del`,
//! `RequestMakeMacro`/`DeleteMacro`, and the `ShortCuts.updateShortCuts`
//! skill-upgrade hook. Macro execution is client-side; the only server-side
//! control point is registration, which is where the no-recurring-macros
//! deviation lives (see `handle_request_make_macro`).

use crate::model::components::{Macros, Shortcuts};
use crate::model::shortcut::{MacroType, MacroUpdateType, Shortcut, ShortcutType};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids, system_message_with};
use crate::session::ClientSession;
use crate::world::World;

/// The in-game player behind a client, or `None` (Java's `getPlayer() ==
/// null` gate).
fn ingame_object_id(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

fn send(world: &World, client_id: u32, body: Vec<u8>) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(body);
    }
}

/// Port of `clientpackets/RequestShortCutReg.runImpl` +
/// `ShortCuts.registerShortCut`: verify ITEM slots against the inventory
/// (a missing item isn't stored), store + persist, then the echo
/// `ShortCutRegister` and a `SkillList` re-send — both unconditional in Java,
/// even when the registry rejected the slot.
pub(crate) fn handle_request_short_cut_reg(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestShortCutReg::read(body) else { return };
    if !(0..=19).contains(&pkt.page) {
        return;
    }
    let Some(object_id) = ingame_object_id(world, client_id) else { return };

    let mut sc = Shortcut {
        slot: pkt.slot,
        page: pkt.page,
        kind: pkt.kind,
        id: pkt.id,
        level: pkt.level,
        character_type: pkt.character_type,
        shared_reuse_group: -1,
    };
    let mut store = true;
    if sc.kind == ShortcutType::Item {
        let exists = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&object_id)
            .is_some_and(|inv| inv.items().iter().any(|i| i.object_id == sc.id));
        if exists {
            // `item.getSharedReuseGroup()` — the template default (0);
            // `shared_reuse_group` is never set in this dist's item XMLs.
            sc.shared_reuse_group = 0;
        } else {
            store = false;
        }
    }
    if store {
        if let Some(shortcuts) = world.objects.get_component_mut::<Shortcuts>(&object_id) {
            shortcuts.put(sc);
        }
    }

    send(world, client_id, server_packets::shortcut_register(&sc));
    if let Some(pkt) = super::helpers::skill_list_packet(world, object_id) {
        send(world, client_id, pkt);
    }
}

/// Port of `clientpackets/RequestShortCutDel.runImpl`.
pub(crate) fn handle_request_short_cut_del(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestShortCutDel::read(body) else { return };
    if !(0..=19).contains(&pkt.page) {
        return;
    }
    let Some(object_id) = ingame_object_id(world, client_id) else { return };
    delete_shortcut(world, client_id, object_id, pkt.slot, pkt.page);
}

/// Port of `ShortCuts.deleteShortCut`: remove + persist, then re-send the
/// whole panel (the client needs no per-slot confirmation — Java sends a
/// fresh `ShortCutInit`; there's no delete packet). The auto-soulshot
/// deactivation branch is skipped — no auto-soulshot system.
fn delete_shortcut(world: &mut World, client_id: u32, object_id: i32, slot: i32, page: i32) {
    let removed = world
        .objects
        .get_component_mut::<Shortcuts>(&object_id)
        .and_then(|shortcuts| shortcuts.remove(slot, page));
    if removed.is_none() {
        return;
    }
    if let Some(shortcuts) = world.objects.get_component::<Shortcuts>(&object_id) {
        send(world, client_id, server_packets::shortcut_init(shortcuts));
    }
}

/// Port of `clientpackets/RequestMakeMacro.runImpl` +
/// `MacroList.registerMacro`, with one deliberate deviation: macros carrying
/// a `SHORTCUT`-type command are rejected (SM 810 "Invalid macro"). That
/// command type ("press panel slot X") is what enables the classic
/// recurring/looping AFK macro — a macro whose last command presses a slot
/// holding a macro, possibly its own — and since execution is client-side,
/// refusing to register it is the only enforcement point. Java accepts them.
pub(crate) fn handle_request_make_macro(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestMakeMacro::read(body) else { return };
    let Some(object_id) = ingame_object_id(world, client_id) else { return };

    let reject = |world: &World, sm_id: i16| send(world, client_id, system_message_with(sm_id, &[]));
    if pkt.commands_length > 255 {
        return reject(world, sm_ids::INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS);
    }
    let macro_count = world.objects.get_component::<Macros>(&object_id).map_or(0, |m| m.entries.len());
    if macro_count > 48 {
        return reject(world, sm_ids::YOU_MAY_CREATE_UP_TO_48_MACROS);
    }
    if pkt.macro_.name.is_empty() {
        return reject(world, sm_ids::ENTER_THE_NAME_OF_THE_MACRO);
    }
    if pkt.macro_.descr.chars().count() > 32 {
        return reject(world, sm_ids::MACRO_DESCRIPTIONS_MAY_CONTAIN_UP_TO_32_CHARACTERS);
    }
    // The no-recurring-macros deviation (see the fn doc).
    if pkt.macro_.commands.iter().any(|c| c.kind == MacroType::Shortcut) {
        return reject(world, sm_ids::INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS);
    }

    let Some(macros) = world.objects.get_component_mut::<Macros>(&object_id) else { return };
    let (id, update) = macros.register(pkt.macro_);
    let Some(registered) = macros.get(id).cloned() else { return };
    send(world, client_id, server_packets::send_macro_list(1, Some(&registered), update));
}

/// Port of `clientpackets/RequestDeleteMacro.runImpl` +
/// `MacroList.deleteMacro`: remove + persist, cascade-delete every panel
/// slot holding the macro, then the DELETE echo. Java runs the shortcut
/// cascade even for an unknown macro id; the echo is skipped for one (the
/// null macro NPEs in Java's `writeImpl`, so no packet reaches the client
/// there either).
pub(crate) fn handle_request_delete_macro(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestDeleteMacro::read(body) else { return };
    let Some(object_id) = ingame_object_id(world, client_id) else { return };

    let removed = world.objects.get_component_mut::<Macros>(&object_id).and_then(|m| m.delete(pkt.id));

    let slots = world
        .objects
        .get_component::<Shortcuts>(&object_id)
        .map(|s| s.slots_of_macro(pkt.id))
        .unwrap_or_default();
    for (slot, page) in slots {
        delete_shortcut(world, client_id, object_id, slot, page);
    }

    if let Some(removed) = removed {
        send(world, client_id, server_packets::send_macro_list(0, Some(&removed), MacroUpdateType::Delete));
    }
}

/// Port of `ShortCuts.updateShortCuts` — a learned/auto-granted skill
/// upgrade rewrites every SKILL slot holding it (new level, `ShortCutRegister`
/// echo per slot, row upsert). Called from `RequestAcquireSkill` and the
/// level-up `rewardSkills` grants.
pub(crate) fn update_skill_shortcuts(world: &mut World, object_id: i32, skill_id: i32, skill_level: i32) {
    let Some(shortcuts) = world.objects.get_component_mut::<Shortcuts>(&object_id) else { return };
    let mut updated = Vec::new();
    for sc in shortcuts.0.values_mut() {
        if sc.kind == ShortcutType::Skill && sc.id == skill_id {
            sc.level = skill_level;
            updated.push(*sc);
        }
    }
    if updated.is_empty() {
        return;
    }
    let client_id = super::helpers::client_for_player(world, object_id);
    for sc in updated {
        if let Some(client_id) = client_id {
            send(world, client_id, server_packets::shortcut_register(&sc));
        }
    }
}

/// Port of the `Player.removeSkill` shortcut cascade: drop every panel slot
/// holding `skill_id` as a SKILL shortcut (transform skills 3080–3259 are left
/// in place, per Java), persist each deletion, and re-send the panel once
/// (Java's `deleteShortCut` → `ShortCutInit`). Used when a delevel removes a
/// skill outright.
pub(crate) fn remove_skill_shortcuts(world: &mut World, object_id: i32, skill_id: i32) {
    if (3080..=3259).contains(&skill_id) {
        return;
    }
    let Some(shortcuts) = world.objects.get_component::<Shortcuts>(&object_id) else { return };
    let victims: Vec<(i32, i32)> = shortcuts
        .0
        .values()
        .filter(|sc| sc.kind == ShortcutType::Skill && sc.id == skill_id)
        .map(|sc| (sc.slot, sc.page))
        .collect();
    if victims.is_empty() {
        return;
    }
    if let Some(shortcuts) = world.objects.get_component_mut::<Shortcuts>(&object_id) {
        for &(slot, page) in &victims {
            shortcuts.remove(slot, page);
        }
    }
    if let (Some(client_id), Some(shortcuts)) =
        (super::helpers::client_for_player(world, object_id), world.objects.get_component::<Shortcuts>(&object_id))
    {
        send(world, client_id, server_packets::shortcut_init(shortcuts));
    }
}
