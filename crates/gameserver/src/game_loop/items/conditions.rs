//! Item conditions — Java `ItemTemplate.checkCondition`, over the `<cond>`
//! trees [`crate::data::item_cond`] parses.
//!
//! # What the function actually is
//!
//! Three gates and a loop, in this order:
//!
//! 1. a GM holding `PlayerCondOverride.ITEM_CONDITIONS` passes everything,
//!    unless `GMItemRestriction` (this dist: **True**) puts them back under the
//!    rules — the key whose only job is this branch, and which had nothing to
//!    gate until now;
//! 2. an Olympiad-restricted or hero item is refused *inside a match*, with a
//!    different message for equip and for use;
//! 3. an event-restricted item is refused while the holder is on an event;
//! 4. then each `<cond>` block in document order, first failure winning.
//!
//! # Who asks
//!
//! Java's signature is `checkCondition(Creature creature, WorldObject object,
//! boolean sendMessage)`, and **every one of its four call sites passes the
//! same object twice** — `UseItem` (player, player), `RequestPetUseItem` (pet,
//! pet), `Player.checkItemRestriction` (this, this) and `PetInventory.restore`
//! (owner, owner). So `effected` is always `effector`, and the one `<target>`
//! condition on this dist (item 21746's `levelRange`) asks about the user like
//! every `<player>` one does. This takes a single object id for that reason.
//!
//! The effector is **not** always a player. A pet uses items through its own
//! window, and the conditions split three ways on that:
//!
//! * `ConditionPlayerRace` and friends test `effector.isPlayer()` — false for a
//!   pet, so a race-gated item is refused for it;
//! * the majority read `effector.getActingPlayer()`, which for a summon is its
//!   **owner** — so a pet armour gated on `sex` reads the owner's sex;
//! * `ConditionCategoryType` reads `effector.getId()`, which is a player's
//!   *class* id but a summon's *npc* id. That is what makes
//!   `categoryType="STRIDER"` mean "the wearer is a strider", and it is why
//!   pet gear is gated this way at all.
//!
//! The three states are [`Effector`] below.

use crate::data::item_cond::{Cond, CondMessage};
use crate::data::item_data::ItemTemplate;
use crate::game_loop::helpers::{instance_of, level_of, npc_id_of, player_race, send_sm_to_player};
use crate::model::Player;
use crate::model::castle::CastleSide;
use crate::model::components::ServitorOf;
use crate::network::server_packets::{SmParam, sm_ids};
use crate::world::World;

/// Java `ItemTemplate.checkCondition(creature, object, sendMessage)`.
///
/// `object_id` is the creature using or wearing the item — a player, or a
/// pet/servitor acting through its own inventory.
pub(crate) fn check_condition(
    world: &World,
    object_id: i32,
    template: &ItemTemplate,
    send_message: bool,
) -> bool {
    let effector = Effector::of(world, object_id);

    // "creature.canOverrideCond(ITEM_CONDITIONS) && !Config.GM_ITEM_RESTRICTION"
    if !world.cfg.general.gm_item_restriction
        && world
            .objects
            .get_component::<Player>(&object_id)
            .is_some_and(|p| p.can_override_cond(crate::game_loop::admin::ITEM_CONDITIONS_ORDINAL))
    {
        return true;
    }

    // "Don't allow hero equipment and restricted items during Olympiad."
    // `isOlyRestrictedItem()` is the template flag OR'd with the (empty)
    // `AltOlyRestrictedItems` list; `_heroItem` is an id range, not a flag.
    if (is_oly_restricted(world, template) || is_hero_item(template.item_id))
        && effector.player.is_some_and(|p| in_olympiad_mode(world, p))
    {
        send_sm(
            world,
            object_id,
            if template.is_equipable() {
                sm_ids::YOU_CANNOT_EQUIP_THAT_ITEM_IN_A_OLYMPIAD_MATCH
            } else {
                sm_ids::YOU_CANNOT_USE_THAT_ITEM_IN_A_OLYMPIAD_MATCH
            },
            &[],
        );
        return false;
    }

    if template.is_event_restricted && effector.player.is_some_and(|p| on_event(world, p)) {
        // Java's plain `sendMessage`, which is `S1_TEXT` on the wire.
        send_sm(
            world,
            object_id,
            sm_ids::S1_TEXT,
            &[SmParam::Text(
                "You cannot use this item in the event.".into(),
            )],
        );
        return false;
    }

    for condition in &template.pre_conditions {
        if eval(world, &effector, &condition.node) {
            continue;
        }
        // Java answers a **summon** with its own line and ignores
        // `sendMessage` entirely — the pet windows call with `false` and would
        // otherwise refuse silently.
        if effector.is_summon {
            send_sm(world, object_id, sm_ids::THIS_PET_CANNOT_USE_THIS_ITEM, &[]);
            return false;
        }
        if send_message {
            send_refusal(world, object_id, template.item_id, &condition.message);
        }
        return false;
    }
    true
}

/// Java `ItemTemplate.isConditionAttached()` — used by `RequestPetUseItem` as a
/// gate in its own right: an equippable item with **no** `<cond>` is not pet
/// gear, and the pet is told so.
pub(crate) fn is_condition_attached(template: &ItemTemplate) -> bool {
    !template.pre_conditions.is_empty()
}

/// The creature a condition is evaluated against, resolved once.
///
/// Java re-derives `getActingPlayer()` inside every condition; the answers do
/// not change within one evaluation, so they are read once here. The three
/// fields are exactly the three questions the condition classes ask.
struct Effector {
    /// The object id itself — `getLevel()`, zones and position read this.
    object_id: i32,
    /// `getActingPlayer()`: the player themself, or a summon's **owner**.
    /// `None` for a creature with no owning player at all.
    player: Option<i32>,
    /// `isSummon()` — also the "`isPlayer()` is false" case the race, hero and
    /// clan conditions refuse outright.
    is_summon: bool,
}

impl Effector {
    fn of(world: &World, object_id: i32) -> Self {
        let owner = world
            .objects
            .get_component::<ServitorOf>(&object_id)
            .map(|s| s.owner_object_id);
        let is_player = world.objects.get_component::<Player>(&object_id).is_some();
        Self {
            object_id,
            player: if is_player { Some(object_id) } else { owner },
            is_summon: !is_player && owner.is_some(),
        }
    }

    /// `effector.isPlayer()` — the conditions that refuse a pet outright.
    fn is_player(&self) -> bool {
        !self.is_summon && self.player.is_some()
    }
}

/// `Condition.test` for one node.
fn eval(world: &World, who: &Effector, node: &Cond) -> bool {
    match node {
        Cond::And(cs) => cs.iter().all(|c| eval(world, who, c)),
        Cond::Or(cs) => cs.iter().any(|c| eval(world, who, c)),
        Cond::Not(c) => !eval(world, who, c),

        // `ConditionPlayerRace`: a non-player effector fails outright.
        Cond::Race(races) => {
            who.is_player() && player_race(world, who.object_id).is_some_and(|r| races.contains(&r))
        }
        // `ConditionPlayerLevel` / `ConditionPlayerLevelRange` read the
        // **effector's** level (`Creature.getLevel`), so a pet answers with
        // its own — and `ConditionTargetLevelRange` asks the same creature,
        // since every call site passes it as the target too.
        Cond::Level(min) => level_of(world, who.object_id).is_some_and(|l| l >= *min),
        Cond::LevelRange(lo, hi) | Cond::TargetLevelRange(lo, hi) => {
            level_of(world, who.object_id).is_some_and(|l| l >= *lo && l <= *hi)
        }
        // `ConditionPlayerState(CHAOTIC)`: reputation below zero. With no
        // acting player Java returns `!required`.
        Cond::Chaotic(required) => match who.with_player(world, |p| p.reputation < 0) {
            Some(chaotic) => chaotic == *required,
            None => !*required,
        },
        Cond::IsHero(required) => who.with_player(world, |p| p.is_hero) == Some(*required),
        // Inclusive **maximum**, despite the name (`getPkKills() <= _pk`).
        Cond::PkCount(max) => who
            .with_player(world, |p| p.pk_kills <= *max)
            .unwrap_or(false),
        Cond::SiegeZone { value, .. } => siege_zone(world, who, *value),
        Cond::IsClanLeader(required) => {
            who.player.map(|p| is_clan_leader(world, p)) == Some(*required)
        }
        Cond::PledgeClass(min) => pledge_class(world, who, *min),
        Cond::HasClanHall(halls) => match clan_of(world, who) {
            // No clan: Java's answer is "the list is exactly [0]".
            None => halls.len() == 1 && halls[0] == 0,
            Some(clan_id) => {
                let hideout = hideout_of(world, clan_id);
                if halls.len() == 1 && halls[0] == -1 {
                    hideout > 0
                } else {
                    halls.contains(&hideout)
                }
            }
        },
        // No fortresses on this chronicle, so `getFortId()` is 0 for every
        // clan — see the module header of `data::item_cond`.
        Cond::HasFort(fort) => match clan_of(world, who) {
            None => *fort == 0,
            Some(_) => *fort != -1 && *fort == 0,
        },
        Cond::HasCastle(castle) => match clan_of(world, who) {
            None => *castle == 0,
            Some(clan_id) => {
                let owned = castle_of(world, clan_id);
                if *castle == -1 {
                    owned > 0
                } else {
                    owned == *castle
                }
            }
        },
        Cond::Sex(sex) => who.with_player(world, |p| i32::from(p.is_female) == *sex) == Some(true),
        // Java returns **true** when there is no acting player at all.
        Cond::FlyMounted(required) => who
            .with_player(world, |p| p.is_flying() == *required)
            .unwrap_or(true),
        Cond::VehicleMounted(required) => match who.player {
            None => true,
            Some(p) => in_vehicle(world, p) == *required,
        },
        Cond::ClassIdRestriction(class_ids) => who
            .with_player(world, |p| class_ids.contains(&p.class_id))
            .unwrap_or(false),
        Cond::Subclass(required) => who
            .with_player(world, |p| (p.class_index != 0) == *required)
            .unwrap_or(true),
        Cond::InstanceId(template_ids) => match who.player {
            None => false,
            Some(p) => world
                .instances
                .get(instance_of(world, p))
                .is_some_and(|i| template_ids.contains(&i.template_id)),
        },
        // `PlayerStat._cloakSlot`, whose setter has no caller in the Java
        // tree: false for everyone, on both sides.
        Cond::CloakStatus(required) => who.player.is_some() && !*required,
        // Gated on there being an acting player, but read at the **effector's**
        // position (`ZoneManager.getZones(effector)`).
        Cond::InsideZoneId(zone_ids) => {
            who.player.is_some() && in_any_zone(world, who.object_id, zone_ids)
        }
        // `Creature.isInCategory` → `CategoryData.isInCategory(type, getId())`:
        // a player's class id, a summon's npc id.
        Cond::CategoryType(names) => match category_id(world, who) {
            None => false,
            Some(id) => names
                .iter()
                .any(|name| world.data.categories.contains(name, id)),
        },
        // `Player.getPlayerSide()`: NEUTRAL without a clan or a castle.
        Cond::IsOnSide(side) => who.is_player() && side_of(world, who) == *side,
        Cond::MinimumVitalityPoints(min) => who
            .with_player(world, |p| p.vitality_points >= *min)
            .unwrap_or(false),
    }
}

impl Effector {
    /// Read something off the acting player, or `None` when there is none —
    /// the shape every `getActingPlayer()`-based condition opens with, and
    /// each one decides for itself what `None` means.
    fn with_player<T>(&self, world: &World, f: impl FnOnce(&Player) -> T) -> Option<T> {
        self.player
            .and_then(|oid| world.objects.get_component::<Player>(&oid))
            .map(f)
    }
}

/// `ConditionSiegeZone.testImpl` + `checkIfOk`, castle branch. There are no
/// forts here, so the fort branch — which reads the same bits one nibble up —
/// can only be reached through a fort zone that does not exist.
fn siege_zone(world: &World, who: &Effector, value: i32) -> bool {
    /// `ConditionSiegeZone.COND_*`.
    const COND_NOT_ZONE: i32 = 0x0001;
    const COND_CAST_ATTACK: i32 = 0x0002;
    const COND_CAST_DEFEND: i32 = 0x0004;
    const COND_CAST_NEUTRAL: i32 = 0x0008;

    let Some(player_oid) = who.player.filter(|_| who.is_player()) else {
        // `checkIfOk` refuses a non-player before it looks at anything else;
        // with no castle *and* no fort, the answer is the NOT_ZONE bit.
        return crate::game_loop::pvp::active_siege_castle(world, who.object_id).is_none()
            && (value & COND_NOT_ZONE) != 0;
    };
    // `CastleManager.getCastle(target)` + `castle.getZone().isActive()`: the
    // castle whose siege zone the creature stands in, with its siege running.
    let Some(castle_id) = crate::game_loop::pvp::active_siege_castle(world, player_oid) else {
        return (value & COND_NOT_ZONE) != 0;
    };
    let state = world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| p.siege_state)
        .unwrap_or(0);
    let registered = clan_of(world, who).is_some_and(|clan_id| {
        world
            .sieges
            .get(&castle_id)
            .is_some_and(|s| s.is_registered(clan_id))
    });
    if (value & COND_CAST_ATTACK) != 0 && registered && state == 1 {
        return true;
    }
    if (value & COND_CAST_DEFEND) != 0 && registered && state == 2 {
        return true;
    }
    (value & COND_CAST_NEUTRAL) != 0 && state == 0
}

/// `ConditionPlayerPledgeClass`: a clan is required, a **leader** passes any
/// value, and `-1` means leaders only.
fn pledge_class(world: &World, who: &Effector, min: i32) -> bool {
    let Some(player_oid) = who.player else {
        return false;
    };
    if clan_of(world, who).is_none() {
        return false;
    }
    let leader = is_clan_leader(world, player_oid);
    if min == -1 && !leader {
        return false;
    }
    leader
        || world
            .objects
            .get_component::<Player>(&player_oid)
            .is_some_and(|p| i32::from(p.pledge_class) >= min)
}

/// The acting player's clan id, or `None` when there is no player or no clan.
fn clan_of(world: &World, who: &Effector) -> Option<i32> {
    who.with_player(world, |p| p.clan_id)
        .filter(|id| *id != 0)
        .filter(|id| world.clans.contains_key(id))
}

/// `Player.isClanLeader()`.
fn is_clan_leader(world: &World, player_oid: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&player_oid)
        .and_then(|p| world.clans.get(&p.clan_id))
        .is_some_and(|c| c.leader_id == player_oid)
}

fn hideout_of(world: &World, clan_id: i32) -> i32 {
    world
        .clan_halls
        .values()
        .find(|h| h.owner_id == clan_id)
        .map(|h| h.id)
        .unwrap_or(0)
}

fn castle_of(world: &World, clan_id: i32) -> i32 {
    world.clans.get(&clan_id).map(|c| c.castle_id).unwrap_or(0)
}

/// `Player.getPlayerSide()` — the side of the castle the clan owns.
fn side_of(world: &World, who: &Effector) -> CastleSide {
    let Some(clan_id) = clan_of(world, who) else {
        return CastleSide::Neutral;
    };
    let castle_id = castle_of(world, clan_id);
    world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .map(|c| c.side)
        .unwrap_or(CastleSide::Neutral)
}

fn in_vehicle(world: &World, player_oid: i32) -> bool {
    world
        .objects
        .has_component::<crate::model::boat::InVehicle>(&player_oid)
}

fn in_any_zone(world: &World, object_id: i32, zone_ids: &[i32]) -> bool {
    let Some(pos) = crate::game_loop::helpers::maybe_position(world, object_id) else {
        return false;
    };
    world
        .data
        .zone_data
        .zones_at(pos.x, pos.y, pos.z)
        .any(|z| zone_ids.contains(&z.id))
}

/// `Creature.getId()` as `isInCategory` uses it.
fn category_id(world: &World, who: &Effector) -> Option<i32> {
    if let Some(p) = world.objects.get_component::<Player>(&who.object_id) {
        return Some(p.class_id);
    }
    npc_id_of(world, who.object_id)
}

/// `ItemTemplate.isOlyRestrictedItem()` — the flag OR the config list.
fn is_oly_restricted(world: &World, template: &ItemTemplate) -> bool {
    template.is_oly_restricted
        || world
            .cfg
            .olympiad
            .restricted_items
            .contains(&template.item_id)
}

/// `ItemTemplate._heroItem` — an id range computed in the constructor, not a
/// datapack flag: the Interlude hero weapons (6611-6621), the hero circlet
/// (6842) and the three later hero accessories (9388-9390).
fn is_hero_item(item_id: i32) -> bool {
    (6611..=6621).contains(&item_id) || (9388..=9390).contains(&item_id) || item_id == 6842
}

fn in_olympiad_mode(world: &World, player_oid: i32) -> bool {
    world.olympiad.is_in_competition(player_oid)
}

fn on_event(world: &World, player_oid: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| p.on_event)
}

/// The failing block's own message: a literal string, a `SystemMessageId`, or
/// nothing at all.
fn send_refusal(world: &World, object_id: i32, item_id: i32, message: &CondMessage) {
    match message {
        CondMessage::Silent => {}
        CondMessage::Text(text) => send_sm(
            world,
            object_id,
            sm_ids::S1_TEXT,
            &[SmParam::Text(text.clone())],
        ),
        CondMessage::Sm { id, add_name } => {
            let params: &[SmParam] = if *add_name {
                &[SmParam::ItemName(item_id)]
            } else {
                &[]
            };
            send_sm(world, object_id, *id, params);
        }
    }
}

/// Send to whoever is driving `object_id` — a pet's messages go to its owner's
/// client, which is what `player.sendPacket` does at the pet call sites.
fn send_sm(world: &World, object_id: i32, message_id: i16, params: &[SmParam]) {
    let target = world
        .objects
        .get_component::<ServitorOf>(&object_id)
        .map(|s| s.owner_object_id)
        .unwrap_or(object_id);
    send_sm_to_player(world, target, message_id, params);
}

/// Java `Player.checkItemRestriction` — re-run every equipped item's
/// conditions and strip what no longer passes.
///
/// This is the other half of the gate: [`check_condition`] stops an item going
/// **on**, and this takes off what stopped qualifying while worn. Java calls it
/// wherever one of the inputs can change without an equip — a level or class
/// change (`rewardSkills`), a PK (`onPlayerKill`), a pledge-class change, a
/// teleport (`onTeleported`, which is how the zone- and instance-gated items
/// come off), the Olympiad match start, a clan leader handover and the end of a
/// siege.
///
/// Two narrowings, both recorded rather than silent:
///
/// * Java sends a bare `InventoryUpdate`; the port unequips through
///   `unequip_if_worn`, which also refreshes `ExUserInfoEquipSlot`/`UserInfo`.
///   Every other unequip path here does the same, and the client needs the
///   refresh to stop drawing the item.
/// * The cloak `return` is Java's, not a typo: a cloak coming off ends the
///   whole sweep, so a second failing item stays on until the next call. It is
///   reproduced because the message it sends ("your armor set is no longer
///   complete") is the one the cloak condition exists for.
pub(crate) fn check_item_restriction(world: &mut World, player_oid: i32) {
    let Some(client_id) = crate::game_loop::helpers::client_for_player(world, player_oid) else {
        // Not connected: nothing to send, and the next login re-checks.
        return;
    };
    // Snapshot first — the sweep mutates the paperdoll it is walking.
    let worn: Vec<(i32, i32, i32)> = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&player_oid)
        .map(|inv| {
            inv.equipped_items()
                .iter()
                .map(|i| (i.object_id, i.item_id, i.enchant_level))
                .collect()
        })
        .unwrap_or_default();

    for (item_object_id, item_id, enchant) in worn {
        let Some(template) = world.data.item_data.get(item_id) else {
            continue;
        };
        if check_condition(world, player_oid, template, false) {
            continue;
        }
        let is_cloak = template.body_part == crate::data::item_data::SLOT_BACK;
        crate::game_loop::items::unequip_if_worn(world, client_id, player_oid, item_object_id);
        if is_cloak {
            send_sm(
                world,
                player_oid,
                sm_ids::YOUR_CLOAK_HAS_BEEN_UNEQUIPPED_BECAUSE_YOUR_ARMOR_SET_IS_NO_LONGER_COMPLETE,
                &[],
            );
            return;
        }
        if enchant > 0 {
            send_sm(
                world,
                player_oid,
                sm_ids::THE_EQUIPMENT_S1_S2_HAS_BEEN_REMOVED,
                &[SmParam::Int(enchant), SmParam::ItemName(item_id)],
            );
        } else {
            send_sm(
                world,
                player_oid,
                sm_ids::S1_HAS_BEEN_UNEQUIPPED,
                &[SmParam::ItemName(item_id)],
            );
        }
    }
}
