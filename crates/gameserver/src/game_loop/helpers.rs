//! Small send/broadcast/range helpers shared by the packet handlers.

use crate::game_loop::guard::position;
use crate::model;
use crate::model::Player;
use crate::model::components::{Movement, RegionCell, StatModifiers, Vitals};
use crate::model::inventory::Inventory;
use crate::model::npc::Npc;
use crate::model::stats::Stat;
use crate::network::server_packets;
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

/// Java `SkillData.getSkill(id, level)` — a datapack skill, **cloned**.
///
/// The clone is not incidental. Every `apply_*` in the skill pipeline wants
/// `&mut World`, so a borrow of `world.data.skill_data` cannot survive the
/// call; sixty-odd lookup sites all cloned immediately, and each spelled the
/// four-segment path out by hand. This is that, said once.
///
/// Enchanted sub-levels go through `SkillData::get_enchanted` instead — they
/// are a different lookup, not a defaulted argument.
pub(crate) fn skill_by_id(
    world: &World,
    id: i32,
    level: i32,
) -> Option<crate::model::skill::Skill> {
    world.data.skill_data.get(id, level).cloned()
}

/// The object id of the player driven by `client_id`, or `None` when that
/// session is not `InGame` (still logging in, in the lobby, or already gone).
///
/// The inverse of [`client_for_player`], and the first line of nearly every
/// packet handler — Java reaches the same state through `GameClient.getPlayer()`.
pub(crate) fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

/// The world coordinates of any object carrying a [`Position`], or `None` if
/// it has despawned.
///
/// Delegates to the geo layer's own accessor so there is exactly one
/// implementation; `crate::geo` cannot depend on `game_loop`, so the
/// definition has to live down there.
pub(crate) fn pos_of(world: &World, object_id: i32) -> Option<(i32, i32, i32)> {
    crate::geo::distance::position_of(world, object_id)
}

/// Java `Creature.setXYZ` — put an object at `(x, y, z)` by writing its
/// [`Position`] outright.
///
/// **Teleport semantics, not movement.** This changes where the object *is* and
/// nothing else: no region re-index, no knownlist update, no packet. Every
/// caller pairs it with the rest — `set_player_region` /
/// `visibility::update_npc_region`, a `TeleportToLocation` or `FlyToLocation`
/// broadcast, sometimes an instance change — and dropping one of those is how
/// an object ends up visible to the wrong people or invisible to everyone.
/// [`crate::game_loop::position`] is where movement the world watches happen
/// lives.
///
/// A no-op for an object that has left the world.
pub(crate) fn set_position(world: &mut World, object_id: i32, (x, y, z): (i32, i32, i32)) {
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&object_id)
    {
        p.x = x;
        p.y = y;
        p.z = z;
    }
}

/// [`set_position`] that also faces the object — Java's
/// `setXYZ` + `setHeading` pair, which the respawn and summon paths do
/// together because a creature placed without a heading faces due east.
pub(crate) fn set_position_heading(
    world: &mut World,
    object_id: i32,
    (x, y, z): (i32, i32, i32),
    heading: i32,
) {
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&object_id)
    {
        p.x = x;
        p.y = y;
        p.z = z;
        p.heading = heading;
    }
}

/// Halt a creature mid-path and tell everyone where it stopped — Java
/// `Creature.stopMove` followed by the `StopMove` broadcast.
///
/// A no-op for anything that isn't currently moving. Every intent that
/// interrupts a walk (attack, cast, sit, target change) opens with this.
pub(crate) fn stop_movement(world: &mut World, object_id: i32) {
    if !world.objects.has_component::<Movement>(&object_id) {
        return;
    }
    world.objects.remove_component::<Movement>(&object_id);
    if let Some(pos) = position(world, object_id) {
        broadcast_including_self(
            world,
            object_id,
            &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading),
        );
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

/// Whether an object currently stands inside `zone` — Java
/// `ZoneType.isInsideZone(object)`.
///
/// `false` for an object that has left the world. Every caller is sweeping a
/// boss lair for "who is in here", and something with no position is not in
/// here.
///
/// Takes a resolved `&Zone` rather than a zone id because the callers are
/// filters over a region's worth of objects: the lookup is hoisted out of the
/// loop, which is also what makes the missing-zone case theirs to decide. Those
/// that keep an id-based check split on it deliberately —
/// `is_some_and` for "is this player in the boss zone?" (no zone ⇒ no) against
/// `is_none_or` for "has the boss left its lair?" (no zone ⇒ don't drag it
/// back) — so folding them in here would have to pick one and silently change
/// the other.
pub(crate) fn in_zone(world: &World, object_id: i32, zone: &crate::data::zone_data::Zone) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Position>(&object_id)
        .is_some_and(|p| zone.contains(p.x, p.y, p.z))
}

/// A creature's `(current, maximum)` HP, the maximum widened to `f64` so the
/// pair can be divided or compared without a cast at every use.
///
/// `None` once the object has left the world. Callers that only want the ratio
/// should take [`hp_fraction`], which also handles a zero maximum.
pub(crate) fn hp_pair(world: &World, object_id: i32) -> Option<(f64, f64)> {
    world
        .objects
        .get_component::<Vitals>(&object_id)
        .map(|v| (v.cur_hp, v.max_hp as f64))
}

/// A creature's HP as a fraction of its maximum, `0.0..=1.0` — Java's
/// `getCurrentHp() / getMaxHp()`, the number behind every "below N%" gate.
///
/// `None` for a departed object **and** for a zero maximum. That second case is
/// not paranoia: `max_hp` is 0 between an NPC's spawn and its first stat
/// recompute, and dividing there yields a `NaN` that compares `false` against
/// every threshold — so a boss script silently behaves as though the mob were
/// at full health. Every caller guarded it separately, in four different ways,
/// and one had missed it.
///
/// The fraction is canonical rather than a percentage on purpose: what each
/// caller does with a missing answer is *theirs*. `npc_cast` treats it as
/// "healthy, don't heal", `cubic` as "dead, skip", and those are opposite
/// defaults that must not be folded into one helper.
pub(crate) fn hp_fraction(world: &World, object_id: i32) -> Option<f64> {
    world
        .objects
        .get_component::<Vitals>(&object_id)
        .filter(|v| v.max_hp > 0)
        .map(|v| v.cur_hp / v.max_hp as f64)
}

/// Whether a creature counts as dead — **`true` when it has no [`Vitals`] at
/// all**.
///
/// [`Vitals`] is attached once at NPC spawn and player load and is never
/// removed on its own, so "no Vitals" means the object has left the world or
/// was never a creature (a dropped item, a door). Every caller is a
/// "may I still act on this target?" guard, and for those, an object that
/// isn't there must answer the same way a corpse does.
pub(crate) fn is_dead(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Vitals>(&object_id)
        .is_none_or(|v| v.dead)
}
pub(crate) fn is_friend(world: &World, owner_oid: i32, target_oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Friends>(&owner_oid)
        .is_some_and(|fl| fl.0.iter().any(|f| f.char_id == target_oid))
}

pub(crate) fn restore_hp_mp(world: &mut World, object_id: i32) {
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&object_id) {
        v.cur_hp = v.max_hp as f64;
        v.cur_mp = v.max_mp as f64;
    }
}

pub(crate) fn send_inventory_item_list(world: &World, player: i32) {
    // if let (Some(inv), Some(client_id)) = (
    //     world.objects.get_component::<Inventory>(&player),
    //     client_for_player(world, player),
    // ) {
    //     send_to_client(
    //         world,
    //         client_id,
    //         crate::network::enter_world::item_list(inv, &world.data, false),
    //     );
    // }
    if let Some(inv) = world.objects.get_component::<Inventory>(&player) {
        send_to_player(
            world,
            player,
            crate::network::enter_world::item_list(inv, &world.data, false),
        );
    }
}

/// A player's [`Vitals`] and `PlayerVitals`, both copied out.
///
/// `None` unless **both** are present, which is what every caller wants: they
/// are feeding a `StatusUpdate` that carries HP, MP and CP together, and half a
/// gauge set is not a packet worth sending.
///
/// Copied rather than borrowed because every caller then needs `&mut World` to
/// broadcast.
pub(crate) fn vitals_pair(
    world: &World,
    player_oid: i32,
) -> Option<(Vitals, crate::model::components::PlayerVitals)> {
    Some((
        world
            .objects
            .get_component::<Vitals>(&player_oid)
            .copied()?,
        world
            .objects
            .get_component::<crate::model::components::PlayerVitals>(&player_oid)
            .copied()?,
    ))
}

/// The region cell an object is binned into, or `None` once it has left the
/// world.
///
/// The key for [`broadcast_near_region`] and the visibility grids — almost
/// every caller feeds the answer straight to one of those.
///
/// Distinct from [`crate::world::region_of`], which derives a region from raw
/// coordinates; this reads the cell the object is actually registered in.
pub(crate) fn region_cell_of(world: &World, object_id: i32) -> Option<(i32, i32)> {
    world
        .objects
        .get_component::<RegionCell>(&object_id)
        .map(|r| r.0)
}

/// The datapack template behind an NPC object — Java `Npc.getTemplate()`.
///
/// `None` covers both "the object has left the world" and "it is not an NPC at
/// all" (a player, a door, a dropped item), which every caller treats the same:
/// there is nothing to read a template fact off, so bail.
///
/// The template is borrowed out of `world.data`, which is immutable after boot,
/// so the returned reference lives as long as the `&World` rather than as long
/// as the component lookup.
pub(crate) fn npc_template(
    world: &World,
    object_id: i32,
) -> Option<&crate::data::npc_data::NpcTemplate> {
    world
        .objects
        .get_component::<Npc>(&object_id)
        .and_then(|n| n.template(world))
}

/// An NPC's template name, empty when the object is gone or has no template.
///
/// The NPC counterpart of [`player_name_or_empty`] — the pet/servitor persist
/// paths and the summon UI all want a `String` and treat "no template" as no
/// name.
pub(crate) fn npc_name_or_empty(world: &World, object_id: i32) -> String {
    npc_template(world, object_id)
        .map(|t| t.name.clone())
        .unwrap_or_default()
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

/// Send one packet to a connected client — Java `GameClient.sendPacket`.
///
/// A direct `clients` lookup. Prefer this over [`send_to_player`] whenever the
/// handler already holds the client id, which packet handlers always do.
pub(crate) fn send_to_client(world: &World, client_id: u32, packet: Vec<u8>) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// Send one packet to the client driving `player_object_id` — Java
/// `Player.sendPacket`. No-op when that player is offline.
///
/// Keyed by **object id**, resolved through [`client_for_player`]'s reverse
/// index. Both this and [`send_to_client`] are O(1) now; prefer the latter
/// when the client id is already in hand, and reach for this when all you have
/// is the object id (scheduled tasks, effects resolved against a target).
pub(crate) fn send_to_player(world: &World, player_object_id: i32, packet: Vec<u8>) {
    if let Some(cid) = client_for_player(world, player_object_id) {
        send_to_client(world, cid, packet);
    }
}

/// `SystemMessage` to a connected client. Pass `&[]` for a message with no
/// substitution parameters.
pub(crate) fn send_sm_to_client(
    world: &World,
    client_id: u32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    send_to_client(
        world,
        client_id,
        server_packets::system_message_with(message_id, params),
    );
}

/// `SystemMessage` to a player by object id — the scanning counterpart of
/// [`send_sm_to_client`]. Pass `&[]` when the message takes no parameters.
pub(crate) fn send_sm_to_player(
    world: &World,
    player_object_id: i32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    send_to_player(
        world,
        player_object_id,
        server_packets::system_message_with(message_id, params),
    );
}

/// A **bare** `SystemMessage` — one that takes no substitution parameters — to
/// a connected client. Most system messages are bare, so this saves the `&[]`
/// at the call site; reach for [`send_sm_to_client`] when there are params.
pub(crate) fn send_sm_bare_to_client(world: &World, client_id: u32, message_id: i16) {
    send_sm_to_client(world, client_id, message_id, &[]);
}

/// A bare `SystemMessage` to a player by object id — the object-id counterpart
/// of [`send_sm_bare_to_client`].
pub(crate) fn send_sm_bare_to_player(world: &World, player_object_id: i32, message_id: i16) {
    send_sm_to_player(world, player_object_id, message_id, &[]);
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

/// How much adena `object_id` is carrying — Java `Inventory.getAdena`. Zero for
/// anything with no [`Inventory`] at all, which is what every caller wants.
pub(crate) fn adena(world: &World, object_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&object_id)
        .map_or(0, |inv| inv.adena())
}

/// The template id behind an object id, or `None` when there is no [`Npc`]
/// there at all — a player, a dropped item, or an id whose npc has already
/// despawned. Callers that want a sentinel spell it themselves (`.unwrap_or(0)`,
/// `map_or`), since 0 is a legitimate template id in some tables.
pub(crate) fn npc_id_of(world: &World, object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<Npc>(&object_id)
        .map(|npc| npc.npc_id)
}

/// The object id of a **usable** instance of `item_id` the player is carrying,
/// or `None` when they have none.
///
/// "Usable" is the `count > 0` filter: a stack that has been spent down to
/// zero is still in the bag until the next inventory flush, and the auto-use
/// scans must not keep firing at it.
pub(crate) fn carried_item(world: &World, player_oid: i32, item_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|i| i.item_id == item_id && i.count > 0)
                .map(|i| i.object_id)
        })
}

/// The item id of one inventory instance, found by its object id. `None` if the
/// owner has no [`Inventory`] or is not holding that instance — the two cases
/// callers treat alike, since both mean "not theirs to act on".
pub(crate) fn item_id_of(world: &World, owner_object_id: i32, item_object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<Inventory>(&owner_object_id)
        .and_then(|inv| inv.by_object_id(item_object_id).map(|it| it.item_id))
}

/// The additive modifier standing on `stat`, defaulting to the additive
/// identity. Nothing with no [`StatModifiers`] has been buffed, so "absent"
/// and "zero" are the same answer.
pub(crate) fn stat_add(world: &World, object_id: i32, stat: Stat) -> f64 {
    world
        .objects
        .get_component::<StatModifiers>(&object_id)
        .and_then(|m| m.add.get(&stat).copied())
        .unwrap_or(0.0)
}

/// The multiplicative modifier standing on `stat`, defaulting to the
/// multiplicative identity — the [`stat_add`] counterpart, and the reason the
/// two cannot share one function: 0.0 and 1.0 are not interchangeable defaults.
pub(crate) fn stat_mul(world: &World, object_id: i32, stat: Stat) -> f64 {
    world
        .objects
        .get_component::<StatModifiers>(&object_id)
        .and_then(|m| m.mul.get(&stat).copied())
        .unwrap_or(1.0)
}

/// Java `Player.sendInventoryUpdate`: an `InventoryUpdate` never travels alone —
/// it's always followed by the adena counter (`ExAdenaInvenCount`) and the
/// weight bar (`ExUserInfoInvenWeight`), so any inventory change refreshes both.
/// Ported paths that only sent the bare `InventoryUpdate` left the adena display
/// stale (e.g. `//create_coin Adena`). `iu` is the already-built InventoryUpdate.
pub(crate) fn send_inventory_update(
    world: &World,
    player_id: i32,
    changes: Vec<model::inventory::ItemChange>,
) {
    let Some(client_id) = client_for_player(world, player_id) else {
        return;
    };
    let max_load = crate::game_loop::weight::max_load(world, player_id);
    let inventory = world.objects.get_component::<Inventory>(&player_id);
    let iu =
        crate::network::enter_world::inventory_update_changes(&world.data, inventory, &changes);
    let extras = inventory.map(|inv| {
        (
            crate::network::enter_world::ex_adena_inven_count(inv),
            crate::network::enter_world::ex_user_info_inven_weight(
                player_id,
                inv,
                &world.data,
                max_load,
            ),
        )
    });
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(iu);
        if let Some((adena, weight)) = extras {
            cs.send(adena);
            cs.send(weight);
        }
    }
}

/// Snapshot still-carried instances as [`ItemChange::Modified`] — the adapter
/// for the paths that know their delta only as object ids of items that stayed
/// in the bag (equip/unequip, an enchant landing, a mana tick). Ids no longer
/// in the inventory are skipped: nothing coherent can be told to the client
/// about an instance this path believes still exists.
pub(crate) fn modified_changes(
    world: &World,
    owner: i32,
    object_ids: &[i32],
) -> Vec<model::inventory::ItemChange> {
    let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
        return Vec::new();
    };
    object_ids
        .iter()
        .filter_map(|oid| inv.by_object_id(*oid))
        .map(|item| model::inventory::ItemChange::Modified(*item))
        .collect()
}

/// Snapshot the result of `add_inventory_item_tracked` as [`ItemChange`]s:
/// a freshly minted instance becomes `Added` (the client must create the
/// slot), a grown stack `Modified`. Taken *after* any post-add stamping
/// (quest/skill grants set the enchant level between the add and the send),
/// so the packet carries the final state.
pub(crate) fn added_changes(
    world: &World,
    owner: i32,
    added: &[(i32, bool)],
) -> Vec<model::inventory::ItemChange> {
    let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
        return Vec::new();
    };
    added
        .iter()
        .filter_map(|&(oid, is_new)| {
            inv.by_object_id(oid).map(|item| {
                if is_new {
                    model::inventory::ItemChange::Added(*item)
                } else {
                    model::inventory::ItemChange::Modified(*item)
                }
            })
        })
        .collect()
}

/// `add_inventory_item_tracked` + [`added_changes`] in one step, for the
/// gain paths with nothing to stamp between the add and the
/// `InventoryUpdate`. `None` means the add itself failed (object-id pool
/// exhausted), exactly as `add_inventory_item` reports it.
pub(crate) fn add_inventory_item_changes(
    world: &mut World,
    owner: i32,
    item_id: i32,
    count: i64,
) -> Option<Vec<model::inventory::ItemChange>> {
    let added = crate::game_loop::items::add_inventory_item_tracked(world, owner, item_id, count)?;
    Some(added_changes(world, owner, &added))
}

/// The full `SkillList` packet for an in-world player — their skill book plus
/// any transiently-granted clan skills (Java `sendSkillList`). `None` when the
/// object carries no skill book (not a live player). The single funnel every
/// `SkillList` resend goes through, so clan skills never fall off the list.
pub(crate) fn skill_list_packet(world: &World, object_id: i32) -> Option<Vec<u8>> {
    use crate::model::components::{ClanSkills, OptionSkills, SkillBook, SkillEnchants};
    let book = world.objects.get_component::<SkillBook>(&object_id)?;
    let empty = ClanSkills::default();
    let clan = world
        .objects
        .get_component::<ClanSkills>(&object_id)
        .unwrap_or(&empty);
    let no_options = OptionSkills::default();
    let options = world
        .objects
        .get_component::<OptionSkills>(&object_id)
        .unwrap_or(&no_options);
    let no_enchants = SkillEnchants::default();
    let enchants = world
        .objects
        .get_component::<SkillEnchants>(&object_id)
        .unwrap_or(&no_enchants);
    Some(crate::network::enter_world::skill_list(
        book,
        enchants,
        clan,
        options,
        &world.data,
    ))
}

/// Send a fresh `EtcStatusUpdate` to one player, built from their current state
/// (expertise grade penalties + silence/message-refusal), mirroring Java's
/// `sendPacket(new EtcStatusUpdate(this))` which reads it all off the player.
/// This is what redraws the grade-penalty and chat-block icons.
pub(crate) fn send_etc_status_update(world: &World, client_id: u32, object_id: i32) {
    use crate::model::components::{AdminFlags, ExpertisePenalty};
    let ep = world
        .objects
        .get_component::<ExpertisePenalty>(&object_id)
        .copied()
        .unwrap_or_default();
    // Java `EtcStatusUpdate._mask` bit 0x01 = message-refusal OR chat-ban OR
    // silence; the chat-block icon is the union.
    let silence = world
        .objects
        .get_component::<AdminFlags>(&object_id)
        .is_some_and(|f| f.silence)
        || super::punishment::is_chat_banned(world, object_id);
    let charges = world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(0, |p| p.charges);
    let wp = world
        .objects
        .get_component::<crate::model::components::WeightPenalty>(&object_id)
        .map_or(0, |w| w.level);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::etc_status_update(
            charges, wp, ep.weapon, ep.armor, silence,
        ));
    }
}

/// Send `packet` to every in-game player that can see `from_object_id`,
/// excluding the broadcaster — Java `Creature.broadcastPacket(packet)` via
/// The instance (world partition) an object is in (Java
/// `WorldObject.getInstanceId()`) — 0, the overworld, when uninstanced.
pub(crate) fn instance_of(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<crate::model::components::InstanceId>(&object_id)
        .map_or(0, |i| i.0)
}

/// `World.forEachVisibleObject`: only players whose world region is in the
/// broadcaster's 3×3 surrounding-region block **and same instance** receive it.
pub(crate) fn broadcast_to_others(world: &World, from_object_id: i32, packet: &[u8]) {
    // The packet is copied into `Bytes` once and refcounted from there, instead
    // of `to_vec()`-ing it per recipient — a crowded region turned one
    // broadcast into dozens of allocations on the game thread.
    broadcast_to_others_shared(world, from_object_id, bytes::Bytes::copy_from_slice(packet));
}

/// [`broadcast_to_others`] for a payload already in `Bytes` —
/// `broadcast_including_self` shares one buffer between the self-send and the
/// onlookers instead of copying the packet twice.
fn broadcast_to_others_shared(world: &World, from_object_id: i32, shared: bytes::Bytes) {
    use crate::model::components::RegionCell;
    let Some(from) = world.objects.get_component::<RegionCell>(&from_object_id) else {
        return;
    };
    let from_region = from.0;
    let from_instance = instance_of(world, from_object_id);
    // The 3×3 block *is* the recipient set, so walk the region index rather
    // than every connected client. Indexed players without a session (the
    // unattended shops) simply resolve to no client and are skipped, which is
    // what the old session scan did by never seeing them.
    for other_id in world.players_visible_from(from_region) {
        if other_id == from_object_id {
            continue;
        }
        if instance_of(world, other_id) != from_instance {
            continue;
        }
        if let Some(cs) = world
            .clients
            .client_of_player(other_id)
            .and_then(|cid| world.clients.get(&cid))
        {
            cs.send(shared.clone());
        }
    }
}

/// Send `packet` to every in-game player in `instance` whose region cell is
/// adjacent to `region` — the broadcast shape for NPC-originated packets (Java
/// `Npc.broadcastPacket`; NPCs never hold a session, so there is no self/others
/// split), scoped to the source's instance so instanced content stays private
/// (G27). `broadcast_near_region` is this with the overworld (instance 0).
pub(crate) fn broadcast_near_region_in(
    world: &World,
    region: (i32, i32),
    instance: i32,
    packet: &[u8],
) {
    // One `Bytes` for the whole block; see `broadcast_to_others`.
    let shared = bytes::Bytes::copy_from_slice(packet);
    for oid in world.players_visible_from(region) {
        if instance_of(world, oid) != instance {
            continue;
        }
        if let Some(cs) = world
            .clients
            .client_of_player(oid)
            .and_then(|cid| world.clients.get(&cid))
        {
            cs.send(shared.clone());
        }
    }
}

/// [`broadcast_near_region_in`] fixed to the overworld (instance 0) — the shape
/// for NPC packets that only ever originate in the open world (boats, fishing,
/// cursed weapons, town social actions, …).
pub(crate) fn broadcast_near_region(world: &World, region: (i32, i32), packet: &[u8]) {
    broadcast_near_region_in(world, region, 0, packet);
}

/// Round a millisecond duration up to whole 100 ms ticks.
pub(crate) fn ms_to_ticks(ms: i32) -> u64 {
    (ms.max(0) as u64).div_ceil(100)
}

/// Java `client.sendPacket(ActionFailed.STATIC_PACKET)` — the bare "I am not
/// doing that" reply, and the single most-sent packet in the port.
///
/// It is not optional politeness: the client arms a local "request in flight"
/// lock the moment it sends an action, and only a reply releases it. A handler
/// that returns without one leaves the player unable to click anything until
/// the next server packet happens to arrive. Every early return in a request
/// handler owes the client one of these.
///
/// [`send_sm_and_action_failed`] is the variant that explains *why* first.
pub(crate) fn send_action_failed(world: &World, client_id: u32) {
    send_to_client(world, client_id, server_packets::action_failed());
}

/// Send a `SystemMessage` + `ActionFailed` to one client — the standard
/// "request rejected" reply shape all over `Player.useMagic` /
/// `SkillCaster.checkUseConditions`.
pub(crate) fn send_sm_and_action_failed(
    world: &World,
    client_id: u32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(message_id, params));
        cs.send(server_packets::action_failed());
    }
}

/// `npc.broadcastPacket(new NpcSay(npc, NPC_GENERAL, npcStringId))` — an NPC
/// says a line to everyone nearby.
///
/// Lifted out of `QuestCtx` so a **boss script** can use it: the body only ever
/// needed the world and the speaker, and the quest coupling was incidental.
/// `QuestCtx::npc_say` now delegates here.
pub(crate) fn npc_say(world: &World, npc_oid: i32, npc_string_id: i32) {
    npc_say_param(world, npc_oid, npc_string_id, None);
}

/// [`npc_say`] with the line's single `$s1` substitution — Java
/// `broadcastSay(NPC_GENERAL, id, param)`.
///
/// `None` is not the same as `Some("")`: the parameterless packet is a
/// different opcode payload, and a client fed an empty parameter draws the
/// placeholder rather than the line.
pub(crate) fn npc_say_param(world: &World, npc_oid: i32, npc_string_id: i32, param: Option<&str>) {
    let Some(npc) = world.objects.get_component::<Npc>(&npc_oid) else {
        return;
    };
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    let pkt = match param {
        Some(p) => {
            crate::network::server_packets::npc_say_param(npc_oid, npc.npc_id, npc_string_id, p)
        }
        None => crate::network::server_packets::npc_say(npc_oid, npc.npc_id, npc_string_id),
    };
    broadcast_near_region(world, region, &pkt);
}

/// `npc.broadcastSay(NPC_GENERAL, text)` — a literal-text chat bubble.
pub(crate) fn npc_say_text(world: &World, npc_oid: i32, text: &str) {
    let Some(npc) = world.objects.get_component::<Npc>(&npc_oid) else {
        return;
    };
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    let pkt = crate::network::server_packets::npc_say_text(npc_oid, npc.npc_id, text);
    broadcast_near_region(world, region, &pkt);
}

/// Send `packet` to a player's own client (if still connected) and every
/// player that can see them — Java `Creature.broadcastPacket(packet)` with
/// `includeSelf == true`.
pub(crate) fn broadcast_including_self(world: &World, object_id: i32, packet: &[u8]) {
    // One `Bytes` for the mover and every onlooker alike.
    let shared = bytes::Bytes::copy_from_slice(packet);
    if let Some(client_id) = client_for_player(world, object_id)
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(shared.clone());
    }
    broadcast_to_others_shared(world, object_id, shared);
}

/// Fire the held-back action — the tail of Java `SkillCaster.stopCasting`
/// (queued skill → `useMagic`, else `EVT_FINISH_CASTING` → the saved MOVE_TO)
/// and of `EVT_READY_TO_ACT` at swing end. Each replay re-enters the normal
/// handler pipeline, so it re-validates everything exactly like a fresh
/// click. No-op while still busy (casting or mid-swing) or dead — the slot
/// stays for the later stop.
pub(crate) fn run_queued_action(world: &mut World, object_id: i32) {
    use crate::model::components::{AttackState, Casting, QueuedAction};
    let Some(&action) = world.objects.get_component::<QueuedAction>(&object_id) else {
        return;
    };
    if world.objects.has_component::<Casting>(&object_id)
        || world
            .objects
            .get_component::<AttackState>(&object_id)
            .is_some_and(|st| st.attack_end_tick > world.tick)
        || is_dead(world, object_id)
    {
        return;
    }
    world.objects.remove_component::<QueuedAction>(&object_id);
    let Some(client_id) = client_for_player(world, object_id) else {
        return;
    };
    match action {
        QueuedAction::Move { x, y, z } => {
            let Some(cur) = position(world, object_id) else {
                return;
            };
            crate::game_loop::position::intention_move_to(
                world,
                client_id,
                object_id,
                cur,
                (x, y, z),
            );
        }
        QueuedAction::Skill {
            skill_id,
            ctrl,
            shift,
        } => {
            crate::game_loop::skills::cast::use_magic(
                world, client_id, object_id, skill_id, ctrl, shift,
            );
        }
        QueuedAction::UseItem { item_object_id } => {
            crate::game_loop::items::use_equipable_item(
                world,
                client_id,
                object_id,
                item_object_id,
            );
        }
    }
}

/// Java `World.forEachVisibleObject(origin, Creature.class, …)` — every living
/// creature (player **or** NPC) in `origin`'s own region cell or an adjacent
/// one, excluding `origin` itself.
///
/// Java's "visible" is exactly this region-neighbourhood test; there is no
/// line-of-sight or radius term in `forEachVisibleObject`, so none is applied
/// here either. Callers that need a distance or LOS filter add it themselves.
///
/// This is the general neighbour query the `RandomizeHate` deferral in the
/// hate-effects slice was waiting on: `faction_call`'s scan only ever walked
/// NPCs, so a mob could never be pointed at a *player* it wasn't already
/// fighting.
pub(crate) fn visible_creatures(world: &mut World, origin_object_id: i32) -> Vec<i32> {
    use crate::model::components::Vitals;
    let Some(origin) = region_cell_of(world, origin_object_id) else {
        return Vec::new();
    };
    // Both halves come from the region indexes. This used to sweep every
    // entity in the store — all ~34.9k NPCs — and discard the 99.9% that were
    // nowhere near the origin.
    let mut out: Vec<i32> = world
        .players_visible_from(origin)
        .chain(world.npcs_visible_from(origin))
        .filter(|&oid| {
            oid != origin_object_id
                && world
                    .objects
                    .get_component::<Vitals>(&oid)
                    .is_some_and(|v| !v.dead)
        })
        .collect();
    // Sorted so the caller's `Rnd.get(size)` index maps to a stable candidate.
    // Java's iteration order is arbitrary too, and a uniform index over a
    // sorted list is still uniform — but this makes a forced roll in tests
    // pick a *known* creature instead of whatever the ECS happened to yield.
    out.sort_unstable();
    out
}

/// Java `Player.setInventoryBlockingStatus(true)` — suppress inventory
/// refreshes for this player, and schedule the 1500 ms `InventoryEnableTask`
/// that lifts it.
///
/// Called wherever Java calls it: opening a merchant buy list, a private or
/// clan warehouse, and the "wear" (try-on) shop.
pub(crate) fn block_inventory(world: &mut World, object_id: i32) {
    world.inventory_blocked.insert(object_id);
    world.scheduler.schedule(
        world.tick + ms_to_ticks(1500),
        crate::scheduler::ScheduledTask::InventoryEnable { object_id },
    );
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
