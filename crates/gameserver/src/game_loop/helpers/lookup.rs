//! Object/session/name/template lookups.

use crate::game_loop::npc;
use crate::model;
use crate::model::Player;
use crate::model::npc::Npc;
use crate::session::ClientSession;
use crate::world::World;
/// The client id of the in-game session linked to a `Player`, or `None` if
/// they've disconnected since the task was scheduled (dead-id ⇒ no-op, per
/// the scheduler's contract).
///
/// O(1): [`crate::session::ClientTable`] keeps the object-id → client-id
/// reverse index. This used to scan every connected session.
pub(crate) fn client_for_player(world: &World, player_object_id: i32) -> Option<u32> {
    world.clients.client_of_player(player_object_id)
}

pub(crate) fn maybe_object_name(world: &World, oid: i32) -> Option<String> {
    if let Some(p) = world.objects.get_component::<Player>(&oid) {
        return Some(p.name.clone());
    }
    if let Some(npc) = world.objects.get_component::<Npc>(&oid)
        && let Some(t) = world.data.npc_data.get(npc.npc_id)
    {
        return Some(t.name.clone());
    }
    None
}

/// Java `WorldObject.getName()` for GM feedback — player name, else the NPC
/// template name, else the object id.
pub(crate) fn object_name(world: &World, oid: i32) -> String {
    maybe_object_name(world, oid).unwrap_or(oid.to_string())
}

/// The object id of the player driven by `client_id`, or `None` when that
/// session is not `InGame` (still logging in, in the lobby, or already gone).
///
/// The inverse of [`client_for_player`], and the first line of nearly every
/// packet handler — Java reaches the same state through `GameClient.getPlayer()`.
pub(crate) fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}
/// `ClassId.level()` — the occupation tier a class sits at: 0 for a base
/// class, 1/2/3 after the first/second/third transfer.
///
/// Read off the `*_CLASS_GROUP` categories rather than the class id itself,
/// the same mapping the henna slots, the clan-membership gate and the
/// `/dismount`-style user commands all need.
pub(crate) fn class_level(world: &World, class_id: i32) -> i32 {
    let c = &world.data.categories;
    if c.contains("FOURTH_CLASS_GROUP", class_id) {
        3
    } else if c.contains("THIRD_CLASS_GROUP", class_id) {
        2
    } else if c.contains("SECOND_CLASS_GROUP", class_id) {
        1
    } else {
        0
    }
}

/// Argument `n` of a chat command, parsed as `T`. `None` when it is missing
/// **or** does not parse.
///
/// Those two failures are the same failure to every caller — both mean "show
/// the usage line" — and Java's handlers conflate them the same way, wrapping a
/// `countTokens()` check and the parse in one `try`. Bundling them here is what
/// lets a command state its arity as a tuple pattern at its head instead of
/// three lines of `args.get(i).and_then(|s| s.parse::<i32>().ok())` each.
///
/// The turbofish is usually only needed on the first element of such a tuple;
/// the rest infer from the binding.
pub(crate) fn nth_arg<T: std::str::FromStr>(args: &[&str], n: usize) -> Option<T> {
    args.get(n).and_then(|s| s.parse().ok())
}

/// Java `CharInfoTable.getAccessLevelById(id) > 0` — whether a character is a
/// GM.
///
/// Only answerable for someone **in the world**: the port's offline name→id
/// table carries no access level, so an offline GM reads as `false`. That is
/// load-bearing at exactly one call site — the block list, where the
/// consequence is a block row against a GM that the chat filter then honours —
/// and it is recorded here rather than papered over with a DB round-trip on a
/// packet any client can spam.
pub(crate) fn is_gm(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_gm(&world.data))
}

/// Read-modify-write one object's [`AdminFlags`](model::components::AdminFlags),
/// creating the component from its all-false default when absent.
///
/// The systems' half of the GM flags: olympiad observer mode, TvT's freeze and
/// the `//invul`-style toggles all set *one* bit on a component that may or may
/// not be there yet, and must leave every other bit alone — a GM who enters
/// observer mode keeps their `//hide`. Absent and all-false are the same state
/// to every reader (they all go through `is_some_and`/`map_or` on a field), so
/// inserting the default and then flipping the bit is the whole operation.
///
/// A no-op for an object that has left the world, like `add_components` itself.
pub(crate) fn update_admin_flags(
    world: &mut World,
    object_id: i32,
    edit: impl FnOnce(&mut model::components::AdminFlags),
) {
    let mut flags = world
        .objects
        .get_component::<model::components::AdminFlags>(&object_id)
        .copied()
        .unwrap_or_default();
    edit(&mut flags);
    world.objects.add_components(&object_id, flags);
}

/// One object's [`Reuses`](model::components::Reuses) for writing,
/// attaching an empty map first when the object has none. `None` only for an
/// id that has left the world, like `add_components` itself.
///
/// The attach is the point. Players are given `Reuses` at load but **NPCs are
/// not** — the map is attached on first stamp rather than at spawn, so only
/// the creatures that actually cast pay for it (this world holds ~34.9k NPCs,
/// the vast majority of which never cast anything). A plain
/// `get_component_mut` write is therefore a silent no-op on an NPC, and the
/// reuse check reads an absent component as "ready", so the cooldown would
/// never apply at all and a mob could re-cast as fast as its AI ticked.
pub(crate) fn reuses_mut(
    world: &mut World,
    object_id: i32,
) -> Option<&mut model::components::Reuses> {
    if world
        .objects
        .get_component::<model::components::Reuses>(&object_id)
        .is_none()
    {
        world
            .objects
            .add_components(&object_id, model::components::Reuses::default());
    }
    world
        .objects
        .get_component_mut::<model::components::Reuses>(&object_id)
}

/// One entry of a character's [`PlayerVariables`](model::components::PlayerVariables)
/// store — Java `player.getVariables().getString(key, null)`.
///
/// The raw string, so a caller can tell **absent** from a stored `"0"`;
/// [`player_var_int`] folds both into its default. `None` also covers "not in
/// the world", which every reader treats as absent.
pub(crate) fn player_var<'a>(world: &'a World, object_id: i32, key: &str) -> Option<&'a str> {
    world
        .objects
        .get_component::<model::components::PlayerVariables>(&object_id)
        .and_then(|v| v.0.get(key))
        .map(String::as_str)
}

/// Java `player.getVariables().getInt(key, default)` — `default` for an absent
/// character, an absent key, or a value that does not parse.
pub(crate) fn player_var_int(world: &World, object_id: i32, key: &str, default: i32) -> i32 {
    world
        .objects
        .get_component::<model::components::PlayerVariables>(&object_id)
        .map_or(default, |v| v.get_int(key, default))
}

/// Java `player.getVariables().set(key, value)`, memory-first: the map is
/// flushed with the rest of the character by the autosave, so there is no
/// `storeMe` half to call.
///
/// `false` when the character is not in the world — the write went nowhere, and
/// a caller that must land it anyway (the olympiad season roll, which pays
/// offline nobles too) can fall back to a direct `StoreCharVar`.
pub(crate) fn set_player_var(
    world: &mut World,
    object_id: i32,
    key: &str,
    value: impl Into<String>,
) -> bool {
    match world
        .objects
        .get_component_mut::<model::components::PlayerVariables>(&object_id)
    {
        Some(v) => {
            v.0.insert(key.to_string(), value.into());
            true
        }
        None => false,
    }
}

/// [`set_player_var`] for the numeric keys — Java stores them as their decimal
/// string, which is what [`player_var_int`] parses back.
pub(crate) fn set_player_var_int(world: &mut World, object_id: i32, key: &str, value: i32) -> bool {
    set_player_var(world, object_id, key, value.to_string())
}

/// Java `player.getVariables().remove(key)`. A no-op for a character who is
/// not in the world, which no caller distinguishes from "the key was not set".
pub(crate) fn unset_player_var(world: &mut World, object_id: i32, key: &str) {
    if let Some(v) = world
        .objects
        .get_component_mut::<model::components::PlayerVariables>(&object_id)
    {
        v.0.remove(key);
    }
}

/// Java `Creature.isPlayable()` — the `Playable` subtree: a player, their pet,
/// or a summoned servitor. A monster, a guard, or a door is false.
///
/// Note the summons are NPC objects on this side, so an `isPlayable` question
/// can never be answered by "is it an NPC" alone; that is exactly why the three
/// component probes belong in one place rather than being re-spelled per call
/// site.
pub(crate) fn is_playable(world: &World, object_id: i32) -> bool {
    world.objects.has_component::<Player>(&object_id)
        || world
            .objects
            .has_component::<model::components::PetOf>(&object_id)
        || world
            .objects
            .has_component::<model::components::ServitorOf>(&object_id)
}

/// A player's character name, or `None` once the object has left the world.
///
/// Prefer this whenever the caller can say something useful about a missing
/// player; reach for [`player_name_or_empty`] only where Java would have
/// formatted a `null` name into the message anyway.
pub(crate) fn player_name(world: &World, object_id: i32) -> Option<String> {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.name.clone())
}

/// A player's character name, empty when the object has left the world.
///
/// The shape every message-formatting call site wants — `SmParam::Text` and
/// friends take a `String`, and an absent player formats as blank.
pub(crate) fn player_name_or_empty(world: &World, object_id: i32) -> String {
    player_name(world, object_id).unwrap_or_default()
}

/// `DecimalFormat("#,###")` — thousands-grouped integer.
pub(crate) fn format_amount(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if value < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
pub fn player(world: &World, object_id: i32) -> Option<&Player> {
    world.objects.get_component::<Player>(&object_id)
}

/// `Player.getRace()` — the race decoded from the ordinal the component stores.
/// `None` for a non-player object; also for an ordinal outside the enum, which
/// no character-create or load path can produce.
pub(crate) fn player_race(world: &World, object_id: i32) -> Option<crate::enums::Race> {
    player(world, object_id).and_then(|p| crate::enums::Race::from_ordinal(p.race))
}

/// [`player_race`] with the port's established Human fallback, for the callers
/// that need a race to index a table (`MapRegion::town_respawn`) and have no
/// meaningful branch for "no race" — Java's `getRace()` cannot fail there.
pub(crate) fn player_race_or_human(world: &World, object_id: i32) -> crate::enums::Race {
    player_race(world, object_id).unwrap_or(crate::enums::Race::Human)
}

pub(crate) fn level_of(world: &World, object_id: i32) -> Option<i32> {
    if let Some(p) = player(world, object_id) {
        return Some(p.level);
    }
    npc::lvl_of_npc(world, object_id)
}

pub(crate) fn get_others_in_matching_room(
    world: &World,
    room_id: i32,
    player_oid: i32,
) -> Vec<i32> {
    world
        .matching_rooms
        .get(room_id)
        .map(|r| {
            r.all_members()
                .into_iter()
                .filter(|&o| o != player_oid)
                .collect()
        })
        .unwrap_or_default()
}

/// Send `packet` to every in-game player that can see `from_object_id`,
/// excluding the broadcaster — Java `Creature.broadcastPacket(packet)` via
/// The instance (world partition) an object is in (Java
/// `WorldObject.getInstanceId()`) — 0, the overworld, when uninstanced.
pub(crate) fn instance_of(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<model::components::InstanceId>(&object_id)
        .map_or(0, |i| i.0)
}

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
