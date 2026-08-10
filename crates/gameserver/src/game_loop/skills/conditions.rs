//! Skill conditions — Java `Skill.checkCondition` / `checkConditions`
//! (`handlers/skillconditionhandlers/*`), PLAN_G34_SKILL_PARITY.md §S1.
//!
//! Before this module the port enforced exactly **one** skill condition
//! (`OpExistNpc`, inline in [`super::cast`]) and ignored the rest, so 215 of the
//! 758 learnable skills on this dist fired where Java would have refused them:
//! bow skills cast bare-handed, force skills with no charges, party-only skills
//! on strangers, resurrections on the living.
//!
//! # Java's shape, and where it is checked
//!
//! `Player.useMagic` resolves the target *first*, then calls
//! `skill.checkCondition(caster, target)`, which:
//!
//! 1. lets a fake player, or a GM with `PlayerCondOverride.SKILL_CONDITIONS`,
//!    through unconditionally;
//! 2. refuses a **bad** skill cast while mounted, unless the skill is on
//!    `MountEnabledSkillList`;
//! 3. evaluates `GENERAL` then `TARGET`, stopping at the first failure;
//! 4. on failure sends `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` — **except**
//!    when the caster targeted themselves with a bad skill, which is silent.
//!
//! Individual handlers may send their *own* message first (the transform family
//! is the loudest), and Java sends **both**: the specific one, then the generic
//! one. The port previously sent only the specific one from its inline
//! transform block, which is why folding that block in here changes two tests.
//!
//! # Fail-open is still the default for unported names
//!
//! A `<condition>` whose name has no [`SkillCondition`] variant never reaches
//! this module — `build_condition` drops it at parse time and `SkillGaps`
//! records it. That is deliberate: turning an unimplemented condition into a
//! refusal would break more than it fixes. The census test is what stops the
//! remainder from being forgotten.

use crate::data::zone_data::ZoneKind;
use crate::game_loop::abnormal::flags_of;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::{
    is_dead, is_gm, level_of, player, send_action_failed, send_sm_bare_to_client, send_sm_to_client,
};
use crate::model::Player;
use crate::model::components::{OlympiadObserver, PartyRef, ServitorOf, Vitals, ZoneFlags};
use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::model::skill::effect_flag::BLOCK_RESURRECTION;
use crate::model::skill::{AffectType, MountKind, Skill, SkillCondition, Vital};
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

/// Java's `MountType` ordinals as the port stores them on `Player.mount_type`.
const MOUNT_STRIDER: u8 = 1;
const MOUNT_WYVERN: u8 = 2;

/// A refused cast: the extra line the failing condition sends on its own
/// behalf, ahead of the generic "cannot be used due to unsuitable terms".
/// `None` means the generic message is the only one — Java's silent handlers.
pub(crate) struct Refusal(pub Option<RefusalLine>);

/// Most handlers send a `SystemMessageId`; a couple use Java's plain
/// `player.sendMessage`, which is a `S1_TEXT` system message carrying a literal
/// string.
pub(crate) enum RefusalLine {
    Sm(i16),
    Text(&'static str),
}

/// Java `Skill.checkCondition(caster, target)`: GENERAL then TARGET, first
/// failure wins. `target_oid` is the **resolved** cast target, so this must run
/// after target resolution, exactly where `Player.useMagic` calls it.
///
/// Returns `Ok(())` when the cast may proceed.
pub(crate) fn check_cast(
    world: &World,
    caster_oid: i32,
    skill: &Skill,
    target_oid: i32,
) -> Result<(), Refusal> {
    // Java: `creature.canOverrideCond(SKILL_CONDITIONS) && !Config.GM_SKILL_RESTRICTION`.
    // The port has no per-GM condition-override grid, so the whole
    // access-level-is-GM flag stands in for it — the same substitution
    // `admin::` makes everywhere else. `GM_SKILL_RESTRICTION` is off in this
    // dist's `General.ini`, so a GM skips every condition.
    if is_gm(world, caster_oid) {
        return Ok(());
    }
    // `isMounted() && isBad()` — a mounted player may not cast an offensive
    // skill. Java exempts `MountEnabledSkillList`, which is built from
    // `Config.LIST_MOUNT_ENABLED_SKILLS`; that key is absent from this dist's
    // config, so the list is empty and no skill is exempt.
    if skill.is_bad()
        && world
            .objects
            .get_component::<Player>(&caster_oid)
            .is_some_and(Player::is_mounted)
    {
        return Err(Refusal(None));
    }
    for cond in skill
        .conditions
        .iter()
        .chain(skill.target_conditions.iter())
    {
        if let Err(refusal) = eval(world, caster_oid, skill, target_oid, cond) {
            return Err(refusal);
        }
    }
    Ok(())
}

/// `checkConditions(PASSIVE, …)` — whether a passive skill's stat modifiers
/// count right now.
///
/// The passive scope is evaluated from the **stat pipeline**, not the cast
/// path, and that pipeline (`model::conditioned_passive_buffs`) runs on
/// `GameData` + the player's `Inventory` alone, with no `World` in reach: it is
/// called from `Player::from_char` at enter-world, before the object exists.
/// So the passive gate answers only the equipment conditions, which is all the
/// two `<passiveConditions>` blocks on this dist's learnable skills need — Sword
/// /Blunt Weapon Mastery (205) gates on holding a SWORD or BLUNT.
///
/// Any other condition in a passive block **passes**, deliberately: refusing on
/// a condition this can't evaluate would silently delete a player's passive
/// bonuses. Inner Rhythm (428) declares `TargetMyParty` in a passive block,
/// which has no meaning without a target and which Java's own
/// `TargetMyPartySkillCondition` would answer `false` to (null target) — i.e.
/// Java disables that passive outright.
///
/// Not reproduced, and the reason is now stronger than "datapack noise": the
/// condition is the *only* thing standing between Inner Rhythm and the −10 %
/// song/dance MP discount its own description promises, on a skill players
/// train for. A null-target condition on a passive block is a datapack
/// artefact of Java evaluating party conditions with no party context, not a
/// deliberate switch-off — reproducing it would delete an advertised bonus to
/// match what is plainly a Java-side accident.
pub(crate) fn passive_stat_gate(
    skill: &Skill,
    inventory: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> bool {
    skill.passive_conditions.iter().all(|c| match c {
        SkillCondition::EquipWeapon { mask } => {
            weapon_mask_of(inventory, items).is_some_and(|(equipped, _)| equipped & mask != 0)
        }
        SkillCondition::HandedWeapon { mask, two_handed } => weapon_mask_of(inventory, items)
            .is_some_and(|(equipped, lr)| equipped & mask != 0 && lr == *two_handed),
        SkillCondition::EquipShield => {
            inventory.paperdoll_item_id(PaperdollSlot::LHand) != 0
                && items.armor_type(inventory.paperdoll_item_id(PaperdollSlot::LHand))
                    == crate::data::item_data::ArmorType::Shield
        }
        _ => true,
    })
}

/// Send the refusal Java sends: the condition's own message (if any), then the
/// generic one, then `ActionFailed`.
///
/// The generic message is suppressed when the caster aimed a **bad** skill at
/// themselves — Java's `!((creature == object) && isBad())` guard, which exists
/// so the AoE-on-self resolution of an offensive skill doesn't spam the caster.
pub(crate) fn send_refusal(
    world: &World,
    client_id: u32,
    caster_oid: i32,
    skill: &Skill,
    target_oid: i32,
    refusal: &Refusal,
) {
    match &refusal.0 {
        Some(RefusalLine::Sm(sm)) => send_sm_bare_to_client(world, client_id, *sm),
        Some(RefusalLine::Text(text)) => send_sm_to_client(
            world,
            client_id,
            sm_ids::S1_TEXT,
            &[server_packets::SmParam::Text((*text).into())],
        ),
        None => {}
    }
    if !(caster_oid == target_oid && skill.is_bad()) {
        send_sm_to_client(
            world,
            client_id,
            sm_ids::S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS,
            &[server_packets::SmParam::SkillName {
                id: skill.id,
                level: skill.level,
            }],
        );
    }
    send_action_failed(world, client_id);
}

/// `skill` is deliberately unused: every ported condition reads world state
/// only. Java passes it so a handler can name the skill in its message
/// (`OpResurrection`) or re-run its affect scope (`OpSweeper`); neither is
/// reachable here — the skill name goes into the *generic* refusal, which
/// `send_refusal` builds. Kept in the signature so adding such a condition
/// doesn't have to re-thread it.
fn eval(
    world: &World,
    caster: i32,
    _skill: &Skill,
    target: i32,
    cond: &SkillCondition,
) -> Result<(), Refusal> {
    let ok = |b: bool| if b { Ok(()) } else { Err(Refusal(None)) };
    match cond {
        // ---- equipment ---------------------------------------------------
        SkillCondition::EquipWeapon { mask } => {
            ok(weapon_mask(world, caster).is_some_and(|(equipped, _)| equipped & mask != 0))
        }
        SkillCondition::EquipShield => ok(has_shield(world, caster)),
        // Java returns on the **first** type match instead of continuing the
        // loop, so a listed weapon held in the wrong number of hands fails
        // rather than falling through to another entry. With a mask the two
        // are equivalent: match the type, then test the hand count.
        SkillCondition::HandedWeapon { mask, two_handed } => ok(weapon_mask(world, caster)
            .is_some_and(|(equipped, lr_hand)| equipped & mask != 0 && lr_hand == *two_handed)),
        // ---- caster resources --------------------------------------------
        SkillCondition::Encumbered {
            weight_percent,
            slots_percent,
        } => ok(free_percent(world, caster)
            .is_some_and(|(slots, weight)| slots >= *slots_percent && weight >= *weight_percent)),
        SkillCondition::RemainVital {
            vital,
            amount,
            percent,
            affect,
        } => {
            let subject = match affect {
                AffectType::Caster => Some(caster),
                AffectType::Target => Some(target).filter(|t| *t != 0),
                // Java's switch covers CASTER and TARGET only and falls
                // through to `return false`, so BOTH refuses outright.
                AffectType::Both => None,
            };
            ok(subject.is_some_and(|oid| {
                vital_percent(world, oid, *vital).is_some_and(|cur| percent.test(cur, *amount))
            }))
        }
        SkillCondition::EnergySaved { amount } => ok(charges(world, caster) >= *amount),
        // The inverse, and the one with its own message: refuse *at* the cap.
        SkillCondition::EnergyMax { amount } => {
            if charges(world, caster) >= *amount {
                Err(Refusal(Some(RefusalLine::Sm(
                    sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY,
                ))))
            } else {
                Ok(())
            }
        }
        // ---- caster state -------------------------------------------------
        SkillCondition::CanEscape => ok(!super::super::abnormal::cannot_escape(world, caster)),
        SkillCondition::InsideSiegeZone => ok(in_zone(world, caster, ZoneKind::Siege)),
        SkillCondition::NotInUnderwater => ok(!in_zone(world, caster, ZoneKind::Water)),
        SkillCondition::Mounted { kind } => {
            let want = match kind {
                MountKind::Strider => MOUNT_STRIDER,
                MountKind::Wyvern => MOUNT_WYVERN,
            };
            ok(player(world, caster).is_some_and(|p| p.mount_type == want))
        }
        SkillCondition::CheckSex { is_female } => {
            ok(player(world, caster).is_some_and(|p| p.is_female == *is_female))
        }
        SkillCondition::SocialClass { social_class } => {
            ok(player(world, caster).is_some_and(|p| {
                if p.clan_id == 0 {
                    return false;
                }
                let leader = is_clan_leader(world, p);
                // `-1` means leader-only; otherwise a leader passes anyway.
                leader || (*social_class != -1 && p.pledge_type >= *social_class)
            }))
        }
        SkillCondition::CheckLevel { min, max, affect } => {
            let subject = match affect {
                AffectType::Caster => Some(caster),
                // Java's TARGET leg requires a **player**, unlike the vital one.
                AffectType::Target => Some(target).filter(|t| is_player(world, *t)),
                AffectType::Both => None,
            };
            ok(subject
                .and_then(|oid| level_of(world, oid))
                .is_some_and(|lvl| lvl >= *min && lvl <= *max))
        }
        SkillCondition::CanTransform => can_transform(world, caster),
        SkillCondition::CanSummon => ok(can_summon(world, caster)),
        // Java adds `isAlikeDead` and checks `inObserverMode` twice; the
        // duplicate is dropped, the rest is the summon gate minus teleporting.
        SkillCondition::CanSummonCubic => ok(!is_dead(world, caster) && can_summon(world, caster)),
        SkillCondition::CanSummonSiegeGolem | SkillCondition::BuildCamp => {
            ok(siege_deployable(world, caster))
        }
        SkillCondition::CallPc => call_pc(world, caster),
        // ---- target state --------------------------------------------------
        SkillCondition::TargetPc => ok(is_player(world, target)),
        SkillCondition::TargetRace { race } => ok(race_of(world, target) == Some(*race)),
        SkillCondition::TargetMyParty { include_me } => {
            ok(target_in_my_party(world, caster, target, *include_me))
        }
        SkillCondition::ConsumeBody => {
            if is_consumable_corpse(world, target) {
                Ok(())
            } else {
                Err(Refusal(Some(RefusalLine::Sm(sm_ids::INVALID_TARGET))))
            }
        }
        SkillCondition::Unlock => ok(is_door(world, target) || is_chest(world, target)),
        SkillCondition::Resurrection => resurrection(world, caster, target),
        SkillCondition::SkillAcquire {
            skill_id,
            has_learned,
        } => ok(knows_skill(world, target, *skill_id) == *has_learned),
        // `OpSkill` — the *caster's* own book, and the level must match
        // exactly. The negative form is "not at that level", not "absent", so
        // an Ancient Book stays usable at every level below the one it grants.
        SkillCondition::SkillKnown {
            skill_id,
            skill_level,
            has_learned,
        } => {
            let at_level = world
                .objects
                .get_component::<crate::model::components::SkillBook>(&caster)
                .and_then(|b| b.0.get(skill_id).copied())
                == Some(*skill_level);
            ok(at_level == *has_learned)
        }
        // ---- residences ----------------------------------------------------
        SkillCondition::Home { residence } => ok(owns_residence(world, caster, *residence)),
        // ---- target identity -----------------------------------------------
        SkillCondition::TargetDoor { door_ids } => {
            ok(is_door(world, target) && door_ids.contains(&template_id_of(world, target)))
        }
        // Java re-reads `caster.getTarget()` here for a player caster rather
        // than trusting the resolved target — for a `SELF` skill like Nectar
        // (2005) those are different objects, and it is the *selection* the
        // condition is about.
        SkillCondition::TargetNpc { npc_ids } => {
            let actual = if is_player(world, caster) {
                world
                    .objects
                    .get_component::<crate::model::components::TargetRef>(&caster)
                    .and_then(|t| t.0)
            } else {
                Some(target)
            };
            ok(actual.is_some_and(|t| {
                (is_npc(world, t) || is_door(world, t))
                    && npc_ids.contains(&template_id_of(world, t))
            }))
        }
        SkillCondition::Companion { kind } => ok(match kind {
            crate::model::skill::CompanionKind::Pet => is_pet(world, target),
            // `caster.getServitor(target.getObjectId()) != null` — *my*
            // servitor, not merely any summon.
            crate::model::skill::CompanionKind::MySummon => {
                super::super::servitor::servitor_of(world, caster) == Some(target)
            }
        }),
        // `OpAlignment` — `LAWFUL` is reputation >= 0, `CHAOTIC` below it. The
        // `TARGET` form requires an actual player; a monster fails it.
        SkillCondition::Alignment { affect, chaotic } => {
            let test = |oid: i32| {
                world
                    .objects
                    .get_component::<Player>(&oid)
                    .is_some_and(|p| (p.reputation < 0) == *chaotic)
            };
            ok(match affect {
                AffectType::Caster => test(caster),
                AffectType::Target => is_player(world, target) && test(target),
                // `SkillConditionAffectType` has only CASTER and TARGET, and
                // every carrier on this dist declares one of them — this arm
                // exists because the port shares one wider `AffectType` across
                // conditions. Requiring both ends is the strict reading.
                AffectType::Both => test(caster) && is_player(world, target) && test(target),
            })
        }
        // ---- the pre-G34 hold-out -------------------------------------------
        SkillCondition::ExistNpc(c) => {
            let found = super::cast::op_exist_npc_around(world, caster, c);
            ok(if found { c.is_around } else { !c.is_around })
        }
    }
}

/// `CastleManager.getCastleByOwner(clan)` / `ClanHallData.getClanHallByClan`
/// / `FortManager.getFortByOwner` — [`SkillCondition::Home`]'s three arms.
///
/// Ownership only: Java's `OpHome` does **not** accept the siege-defender
/// fallback that `getTeleToLocation` allows, so a defender who owns no castle
/// is refused the blessed scroll even while standing on the ground it would
/// have sent them to.
fn owns_residence(
    world: &World,
    caster: i32,
    residence: crate::model::skill::ResidenceType,
) -> bool {
    use crate::model::skill::ResidenceType;
    let Some(clan_id) = crate::game_loop::guard::clan_of(world, caster) else {
        return false;
    };
    match residence {
        ResidenceType::Castle => world.clans.get(&clan_id).is_some_and(|c| c.castle_id > 0),
        ResidenceType::ClanHall => world.clan_halls.values().any(|h| h.owner_id == clan_id),
        // No fortress system on this chronicle, so this can never pass — which
        // is the same answer Java gives on a server with no forts registered.
        ResidenceType::Fortress => false,
    }
}

/// The **template** id (`WorldObject.getId()`), which for a door is its door id
/// and for an NPC its npc id — not the runtime object id the condition lists
/// would never match.
fn template_id_of(world: &World, object_id: i32) -> i32 {
    if let Some(d) = world
        .objects
        .get_component::<crate::model::door::Door>(&object_id)
    {
        return d.door_id;
    }
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&object_id)
        .map_or(0, |n| n.npc_id)
}

fn is_npc(world: &World, object_id: i32) -> bool {
    world
        .objects
        .has_component::<crate::model::npc::Npc>(&object_id)
}

/// `WorldObject.isPet()` — a collar pet, as opposed to a summoner's servitor.
fn is_pet(world: &World, object_id: i32) -> bool {
    world
        .objects
        .has_component::<crate::model::components::PetOf>(&object_id)
}

// ---------------------------------------------------------------------------
// State readers. Each is one Java accessor; kept separate so a missing piece of
// world state is one obvious `None` rather than a silently-true condition.
// ---------------------------------------------------------------------------

fn is_player(world: &World, object_id: i32) -> bool {
    object_id != 0 && world.objects.has_component::<Player>(&object_id)
}

fn in_zone(world: &World, object_id: i32, kind: ZoneKind) -> bool {
    world
        .objects
        .get_component::<ZoneFlags>(&object_id)
        .is_some_and(|f| f.contains(kind))
}

fn charges(world: &World, object_id: i32) -> i32 {
    player(world, object_id).map_or(0, |p| p.charges)
}

/// `(equipped weapon's type mask, is it two-handed)` — Java's
/// `caster.getActiveWeaponItem()` plus the `SLOT_LR_HAND` body-part test the
/// `Op1hWeapon`/`Op2hWeapon` pair makes.
fn weapon_mask(world: &World, object_id: i32) -> Option<(u32, bool)> {
    let inv = world.objects.get_component::<Inventory>(&object_id)?;
    weapon_mask_of(inv, &world.data.item_data)
}

/// The inventory-only half, shared with [`passive_stat_gate`].
fn weapon_mask_of(
    inv: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> Option<(u32, bool)> {
    let rhand = inv.paperdoll_item_id(PaperdollSlot::RHand);
    if rhand == 0 {
        return None;
    }
    Some((
        items.weapon_type(rhand).mask_bit(),
        items
            .get(rhand)
            .is_some_and(|t| t.body_part & crate::data::item_data::SLOT_LR_HAND != 0),
    ))
}

/// Java `caster.getSecondaryWeaponItem().getItemType() == ArmorType.SHIELD`.
fn has_shield(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.paperdoll_item_id(PaperdollSlot::LHand))
        .filter(|&id| id != 0)
        .is_some_and(|id| {
            world.data.item_data.armor_type(id) == crate::data::item_data::ArmorType::Shield
        })
}

/// `OpEncumbered`'s two headroom percentages, Java's
/// `calcPercent(max, current) = 100 - current*100/max`. Slots use the
/// **non-quest** inventory size.
fn free_percent(world: &World, object_id: i32) -> Option<(i32, i32)> {
    let inv = world.objects.get_component::<Inventory>(&object_id)?;
    let slot_limit = super::super::weight::inventory_limit(world, object_id).max(1);
    let load_limit = super::super::weight::max_load(world, object_id).max(1);
    let used_slots = inv.non_quest_size(&world.data.item_data);
    let load = super::super::weight::total_load(inv, &world.data);
    Some((
        100 - (used_slots as i32 * 100 / slot_limit),
        100 - ((load * 100 / load_limit as i64) as i32),
    ))
}

fn vital_percent(world: &World, object_id: i32, vital: Vital) -> Option<i32> {
    let v = world.objects.get_component::<Vitals>(&object_id)?;
    let (cur, max) = match vital {
        Vital::Hp => (v.cur_hp, v.max_hp as f64),
        Vital::Mp => (v.cur_mp, v.max_mp as f64),
        // CP is the player-only vitals extension, so an NPC has no CP
        // percentage at all — Java's `getCurrentCpPercent` is on `Playable`.
        Vital::Cp => {
            let cp = world
                .objects
                .get_component::<crate::model::components::PlayerVitals>(&object_id)?;
            (cp.cur_cp, cp.max_cp as f64)
        }
    };
    if max <= 0.0 {
        return None;
    }
    Some((cur * 100.0 / max) as i32)
}

/// `Creature.getRace()`: the NPC template's race for a monster, the character
/// race for a player.
fn race_of(world: &World, object_id: i32) -> Option<crate::enums::Race> {
    if object_id == 0 {
        return None;
    }
    if let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&object_id)
    {
        return world
            .data
            .npc_data
            .get(npc.npc_id)
            .and_then(|t| t.race)
            .and_then(crate::enums::Race::from_ordinal);
    }
    player(world, object_id).and_then(|p| crate::enums::Race::from_ordinal(p.race))
}

fn target_in_my_party(world: &World, caster: i32, target: i32, include_me: bool) -> bool {
    if !is_player(world, target) {
        return false;
    }
    let party_of = |oid: i32| world.objects.get_component::<PartyRef>(&oid).map(|p| p.0);
    match party_of(caster) {
        // Java: no party → only self-targeting, and only when includeMe.
        None => include_me && caster == target,
        Some(mine) => {
            let same = party_of(target) == Some(mine);
            if include_me {
                same
            } else {
                same && caster != target
            }
        }
    }
}

/// `ConsumeBodySkillCondition`: a **spawned, dead** monster or summon.
/// "Spawned" is Java's `isSpawned()` — a corpse that has already decayed out of
/// the world is gone, not merely dead.
fn is_consumable_corpse(world: &World, target: i32) -> bool {
    if target == 0 {
        return false;
    }
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&target)
        .and_then(|npc| world.data.npc_data.get(npc.npc_id))
        .is_some_and(|t| t.is_monster())
        && is_dead(world, target)
}

fn is_door(world: &World, target: i32) -> bool {
    target != 0
        && world
            .objects
            .has_component::<crate::model::door::Door>(&target)
}

fn is_chest(world: &World, target: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&target)
        .is_some_and(|npc| {
            world
                .data
                .npc_data
                .get(npc.npc_id)
                .is_some_and(|t| t.type_name == "Chest")
        })
}

fn knows_skill(world: &World, target: i32, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::SkillBook>(&target)
        .is_some_and(|b| b.0.contains_key(&skill_id))
}

fn is_clan_leader(world: &World, p: &Player) -> bool {
    p.clan_id != 0
        && world
            .clans
            .get(&p.clan_id)
            .is_some_and(|c| c.leader_id == p.object_id)
}

/// `CanTransformSkillCondition`, in Java's own order. Each leg has its own
/// message; the generic refusal follows it, which is the behaviour change from
/// the inline block this replaces (that one sent only the specific message).
fn can_transform(world: &World, caster: i32) -> Result<(), Refusal> {
    let Some(p) = player(world, caster) else {
        return Err(Refusal(None));
    };
    // `isAlikeDead() || isCursedWeaponEquipped()` — silent.
    if is_dead(world, caster) || p.cursed_weapon_equipped_id != 0 {
        return Err(Refusal(None));
    }
    if p.sitting {
        return Err(Refusal(Some(RefusalLine::Sm(
            sm_ids::YOU_CANNOT_TRANSFORM_WHILE_SITTING,
        ))));
    }
    if p.transform_id != 0 {
        return Err(Refusal(Some(RefusalLine::Sm(
            sm_ids::YOU_ALREADY_POLYMORPHED_AND_CANNOT_POLYMORPH_AGAIN,
        ))));
    }
    if world
        .objects
        .get_component::<crate::model::components::Speeds>(&caster)
        .is_some_and(|s| s.swimming)
    {
        return Err(Refusal(Some(RefusalLine::Sm(
            sm_ids::YOU_CANNOT_POLYMORPH_INTO_THE_DESIRED_FORM_IN_WATER,
        ))));
    }
    if p.is_mounted() {
        return Err(Refusal(Some(RefusalLine::Sm(
            sm_ids::YOU_CANNOT_TRANSFORM_WHILE_RIDING_A_PET,
        ))));
    }
    // **Deliberate deviation, carried over from the inline block this replaces.**
    // Java has *two* transform gates: `ConditionPlayerCanTransform` (the
    // `DocumentBase` item-condition system) ends with this
    // registered-on-an-event leg, while `CanTransformSkillCondition` — the one
    // a `<skill><conditions>` block actually resolves to — does not. Java would
    // therefore let a TvT entrant transform via a skill. The port keeps the
    // stricter leg on the skill path too rather than silently opening it up;
    // the divergence is here, in one place, instead of implied by a merged
    // block. Java's own text line, not a SystemMessageId.
    if world.events.tvt.player_list.contains(&caster) {
        return Err(Refusal(Some(RefusalLine::Text(
            "You cannot transform while registered on an event.",
        ))));
    }
    Ok(())
}

/// `CanSummonSkillCondition` minus the airship leg (no airships on this dist).
/// Java's `isSpawnProtected`/`isTeleportProtected` are the post-login and
/// post-teleport grace windows; the port models the teleport half only.
fn can_summon(world: &World, caster: i32) -> bool {
    player(world, caster).is_some_and(|p| {
        !p.is_mounted()
            && !p.teleporting
            && !world.objects.has_component::<OlympiadObserver>(&caster)
    })
}

/// `CanSummonSiegeGolemSkillCondition` / `BuildCampSkillCondition` — the same
/// gate twice in Java: alive, uncursed, in a clan, standing in a residence
/// whose siege is in progress, and on the attacker side of it.
fn siege_deployable(world: &World, caster: i32) -> bool {
    let Some(p) = player(world, caster) else {
        return false;
    };
    if is_dead(world, caster) || p.cursed_weapon_equipped_id != 0 || p.clan_id == 0 {
        return false;
    }
    // Java asks `CastleManager.getCastle(player)` / `FortManager.getFort`
    // for the residence the caster stands in, then requires that residence's
    // siege to be running with the caster's clan on the **attacker** side.
    // Here the standing-in-a-residence half is the `SiegeZone` the caster is
    // inside, which carries the `castle_id` the siege is keyed by. Forts have
    // no port, so only the castle leg exists.
    let Some(pos) = maybe_position(world, caster) else {
        return false;
    };
    world
        .data
        .zone_data
        .zones_at(pos.x, pos.y, pos.z)
        .filter(|z| z.kind == ZoneKind::Siege)
        .any(|z| {
            world.sieges.get(&z.castle_id).is_some_and(|s| {
                s.in_progress
                    && s.clans.iter().any(|c| {
                        c.clan_id == p.clan_id
                            && c.kind == crate::model::siege::SiegeClanType::Attacker
                    })
            })
        })
}

/// `OpCallPcSkillCondition` — Summon Friend's caster-side gate.
fn call_pc(world: &World, caster: i32) -> Result<(), Refusal> {
    let Some(p) = player(world, caster) else {
        return Err(Refusal(None));
    };
    if world
        .olympiad
        .matches
        .iter()
        .any(|m| m.player_a == caster || m.player_b == caster)
    {
        return Err(Refusal(Some(RefusalLine::Sm(
            sm_ids::A_USER_PARTICIPATING_IN_THE_OLYMPIAD_CANNOT_USE_SUMMONING_OR_TELEPORTING,
        ))));
    }
    if world.objects.has_component::<OlympiadObserver>(&caster) {
        return Err(Refusal(None));
    }
    // Java also tests `ZoneId.NO_SUMMON_FRIEND` and `JAIL`; the port has
    // neither zone kind, so only the jail *state* is available.
    if p.jailed {
        return Err(Refusal(Some(RefusalLine::Sm(
            sm_ids::YOU_CANNOT_USE_SUMMONING_OR_TELEPORTING_IN_THIS_AREA,
        ))));
    }
    Ok(())
}

/// `OpResurrectionSkillCondition`. Self-targeting always passes (Java returns
/// before looking at anything); otherwise the target must be a dead player who
/// is neither resurrection-blocked nor already looking at a revive prompt.
fn resurrection(world: &World, caster: i32, target: i32) -> Result<(), Refusal> {
    if target == caster {
        return Ok(());
    }
    if target == 0 {
        return Err(Refusal(None));
    }
    // Whose `revive_request` gates this. For a player it's the target itself;
    // for a pet/servitor the flag lives on the **owner** (Java's
    // `player.isRevivingPet()`). Pet and servitor are one branch here because
    // both carry `ServitorOf`. Anything that is neither is not a valid target.
    let request_holder = if player(world, target).is_some() {
        target
    } else {
        world
            .objects
            .get_component::<ServitorOf>(&target)
            .map(|s| s.owner_object_id)
            .ok_or(Refusal(None))?
    };

    if !is_dead(world, target) {
        // Java sends `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` with the skill
        // name on the summon leg, but `RefusalLine` has no skill-name form and
        // the player leg refuses silently — so both refuse silently.
        return Err(Refusal(None));
    }
    if flags_of(world, target) & BLOCK_RESURRECTION != 0 {
        return Err(Refusal(Some(RefusalLine::Sm(sm_ids::REJECT_RESURRECTION))));
    }
    if player(world, request_holder).is_some_and(|p| p.revive_request.is_some()) {
        return Err(Refusal(Some(RefusalLine::Sm(
            sm_ids::RESURRECTION_HAS_ALREADY_BEEN_PROPOSED,
        ))));
    }
    Ok(())
}
