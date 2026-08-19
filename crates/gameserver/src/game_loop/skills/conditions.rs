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
use crate::game_loop::helpers;
use crate::model::components;
use crate::model::skill;

use crate::model::Player;

use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::model::skill::effect_flag::BLOCK_RESURRECTION;

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
    skill: &skill::Skill,
    target_oid: i32,
) -> Result<(), Refusal> {
    // Java: `creature.canOverrideCond(SKILL_CONDITIONS) && !Config.GM_SKILL_RESTRICTION`.
    //
    // **Both halves of this used to be wrong.** The port read "is this an
    // access-level GM" instead of the override — there was no override grid at
    // load — and the comment justifying it stated `GM_SKILL_RESTRICTION` was
    // off in this dist's `General.ini`. It is **True**, which is precisely the
    // value that *stops* the override exempting anyone, so every GM here
    // skipped every skill condition on a server configured to allow none of
    // that.
    //
    // The override is now real (`Player.restore` seeds a GM with the whole
    // exception mask), so this reads the thing Java reads, and `//set_exception 2`
    // turns it off for one GM without touching the config.
    if world
        .objects
        .get_component::<Player>(&caster_oid)
        .is_some_and(|p| p.can_override_cond(crate::game_loop::admin::SKILL_CONDITIONS_ORDINAL))
        && !world.cfg.general.gm_skill_restriction
    {
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
        eval(world, caster_oid, skill, target_oid, cond)?;
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
    skill: &skill::Skill,
    inventory: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> bool {
    skill.passive_conditions.iter().all(|c| match c {
        skill::SkillCondition::EquipWeapon { mask } => {
            weapon_mask_of(inventory, items).is_some_and(|(equipped, _)| equipped & mask != 0)
        }
        skill::SkillCondition::HandedWeapon { mask, two_handed } => {
            weapon_mask_of(inventory, items)
                .is_some_and(|(equipped, lr)| equipped & mask != 0 && lr == *two_handed)
        }
        skill::SkillCondition::EquipShield => {
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
    skill: &skill::Skill,
    target_oid: i32,
    refusal: &Refusal,
) {
    match &refusal.0 {
        Some(RefusalLine::Sm(sm)) => helpers::send_sm_bare_to_client(world, client_id, *sm),
        Some(RefusalLine::Text(text)) => helpers::send_sm_to_client(
            world,
            client_id,
            sm_ids::S1_TEXT,
            &[server_packets::SmParam::Text((*text).into())],
        ),
        None => {}
    }
    if !(caster_oid == target_oid && skill.is_bad()) {
        helpers::send_sm_to_client(
            world,
            client_id,
            sm_ids::S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS,
            &[server_packets::SmParam::SkillName {
                id: skill.id,
                level: skill.level,
            }],
        );
    }
    helpers::send_action_failed(world, client_id);
}

/// `skill` is deliberately unused: every ported condition reads world state
/// only. Java passes it so a handler can name the skill in its message
/// (`OpResurrection`) or re-run its affect scope (`OpSweeper`); neither is
/// reachable here — the skill name goes into the *generic* refusal, which
/// `send_refusal` builds. Kept in the signature so adding such a condition
/// doesn't have to re-thread it.
/// Evaluate one condition on its own — the dispatcher's per-condition step,
/// exposed so a test can exercise a single gate without building a `Skill`
/// around it. `eval` ignores the skill argument for every condition this
/// helper is used with.
#[cfg(test)]
pub(crate) fn check_for_test(
    world: &World,
    caster: i32,
    target: i32,
    conds: &[skill::SkillCondition],
) -> bool {
    let skill = skill::Skill::default();
    conds
        .iter()
        .all(|c| eval(world, caster, &skill, target, c).is_ok())
}

fn eval(
    world: &World,
    caster: i32,
    _skill: &skill::Skill,
    target: i32,
    cond: &skill::SkillCondition,
) -> Result<(), Refusal> {
    let ok = |b: bool| if b { Ok(()) } else { Err(Refusal(None)) };
    match cond {
        // ---- equipment ---------------------------------------------------
        skill::SkillCondition::EquipWeapon { mask } => {
            ok(weapon_mask(world, caster).is_some_and(|(equipped, _)| equipped & mask != 0))
        }
        skill::SkillCondition::EquipShield => ok(has_shield(world, caster)),
        // Java returns on the **first** type match instead of continuing the
        // loop, so a listed weapon held in the wrong number of hands fails
        // rather than falling through to another entry. With a mask the two
        // are equivalent: match the type, then test the hand count.
        skill::SkillCondition::HandedWeapon { mask, two_handed } => ok(weapon_mask(world, caster)
            .is_some_and(|(equipped, lr_hand)| equipped & mask != 0 && lr_hand == *two_handed)),
        // ---- caster resources --------------------------------------------
        skill::SkillCondition::Encumbered {
            weight_percent,
            slots_percent,
        } => ok(free_percent(world, caster)
            .is_some_and(|(slots, weight)| slots >= *slots_percent && weight >= *weight_percent)),
        skill::SkillCondition::RemainVital {
            vital,
            amount,
            percent,
            affect,
        } => {
            let subject = match affect {
                skill::AffectType::Caster => Some(caster),
                skill::AffectType::Target => Some(target).filter(|t| *t != 0),
                // Java's switch covers CASTER and TARGET only and falls
                // through to `return false`, so BOTH refuses outright.
                skill::AffectType::Both => None,
            };
            ok(subject.is_some_and(|oid| {
                vital_percent(world, oid, *vital).is_some_and(|cur| percent.test(cur, *amount))
            }))
        }
        skill::SkillCondition::EnergySaved { amount } => ok(charges(world, caster) >= *amount),
        // The inverse, and the one with its own message: refuse *at* the cap.
        skill::SkillCondition::EnergyMax { amount } => {
            if charges(world, caster) >= *amount {
                Err(Refusal(Some(RefusalLine::Sm(
                    sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY,
                ))))
            } else {
                Ok(())
            }
        }
        // ---- caster state -------------------------------------------------
        skill::SkillCondition::CanEscape => ok(!super::abnormal::cannot_escape(world, caster)),
        skill::SkillCondition::InsideSiegeZone => ok(in_zone(world, caster, ZoneKind::Siege)),
        skill::SkillCondition::NotInUnderwater => ok(!in_zone(world, caster, ZoneKind::Water)),
        skill::SkillCondition::Mounted { kind } => {
            let want = match kind {
                skill::MountKind::Strider => MOUNT_STRIDER,
                skill::MountKind::Wyvern => MOUNT_WYVERN,
            };
            ok(helpers::player(world, caster).is_some_and(|p| p.mount_type == want))
        }
        skill::SkillCondition::CheckSex { is_female } => {
            ok(helpers::player(world, caster).is_some_and(|p| p.is_female == *is_female))
        }
        skill::SkillCondition::SocialClass { social_class } => {
            ok(helpers::player(world, caster).is_some_and(|p| {
                if p.clan_id == 0 {
                    return false;
                }
                let leader = is_clan_leader(world, p);
                // `-1` means leader-only; otherwise a leader passes anyway.
                leader || (*social_class != -1 && p.pledge_type >= *social_class)
            }))
        }
        skill::SkillCondition::CheckLevel { min, max, affect } => {
            let subject = match affect {
                skill::AffectType::Caster => Some(caster),
                // Java's TARGET leg requires a **player**, unlike the vital one.
                skill::AffectType::Target => Some(target).filter(|t| is_player(world, *t)),
                skill::AffectType::Both => None,
            };
            ok(subject
                .and_then(|oid| helpers::level_of(world, oid))
                .is_some_and(|lvl| lvl >= *min && lvl <= *max))
        }
        skill::SkillCondition::CanTransform => can_transform(world, caster),
        skill::SkillCondition::CanSummon => ok(can_summon(world, caster)),
        // Java adds `isAlikeDead` and checks `inObserverMode` twice; the
        // duplicate is dropped, the rest is the summon gate minus teleporting.
        skill::SkillCondition::CanSummonCubic => {
            ok(!helpers::is_dead(world, caster) && can_summon(world, caster))
        }
        skill::SkillCondition::CanSummonSiegeGolem | skill::SkillCondition::BuildCamp => {
            ok(siege_deployable(world, caster))
        }
        skill::SkillCondition::CallPc => call_pc(world, caster),
        skill::SkillCondition::CanUntransform => can_untransform(world, caster),
        // ---- target state --------------------------------------------------
        skill::SkillCondition::TargetPc => ok(is_player(world, target)),
        skill::SkillCondition::TargetRace { race } => ok(race_of(world, target) == Some(*race)),
        skill::SkillCondition::TargetMyParty { include_me } => {
            ok(target_in_my_party(world, caster, target, *include_me))
        }
        skill::SkillCondition::ConsumeBody => {
            if is_consumable_corpse(world, target) {
                Ok(())
            } else {
                Err(Refusal(Some(RefusalLine::Sm(sm_ids::INVALID_TARGET))))
            }
        }
        skill::SkillCondition::Unlock => ok(is_door(world, target) || is_chest(world, target)),
        skill::SkillCondition::Resurrection => resurrection(world, caster, target),
        skill::SkillCondition::SkillAcquire {
            skill_id,
            has_learned,
        } => ok(knows_skill(world, target, *skill_id) == *has_learned),
        // `OpSkill` — the *caster's* own book, and the level must match
        // exactly. The negative form is "not at that level", not "absent", so
        // an Ancient Book stays usable at every level below the one it grants.
        skill::SkillCondition::SkillKnown {
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
        skill::SkillCondition::Home { residence } => ok(owns_residence(world, caster, *residence)),
        // ---- target identity -----------------------------------------------
        skill::SkillCondition::TargetDoor { door_ids } => {
            ok(is_door(world, target) && door_ids.contains(&template_id_of(world, target)))
        }
        // Java re-reads `caster.getTarget()` here for a player caster rather
        // than trusting the resolved target — for a `SELF` skill like Nectar
        // (2005) those are different objects, and it is the *selection* the
        // condition is about.
        skill::SkillCondition::TargetNpc { npc_ids } => {
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
        skill::SkillCondition::Companion { kind } => ok(match kind {
            crate::model::skill::CompanionKind::Pet => is_pet(world, target),
            // `caster.getServitor(target.getObjectId()) != null` — *my*
            // servitor, not merely any summon.
            crate::model::skill::CompanionKind::MySummon => {
                super::super::servitor::servitor_of(world, caster) == Some(target)
            }
        }),
        // `OpAlignment` — `LAWFUL` is reputation >= 0, `CHAOTIC` below it. The
        // `TARGET` form requires an actual player; a monster fails it.
        skill::SkillCondition::Alignment { affect, chaotic } => {
            let test = |oid: i32| {
                world
                    .objects
                    .get_component::<Player>(&oid)
                    .is_some_and(|p| (p.reputation < 0) == *chaotic)
            };
            ok(match affect {
                skill::AffectType::Caster => test(caster),
                skill::AffectType::Target => is_player(world, target) && test(target),
                // `SkillConditionAffectType` has only CASTER and TARGET, and
                // every carrier on this dist declares one of them — this arm
                // exists because the port shares one wider `AffectType` across
                // conditions. Requiring both ends is the strict reading.
                skill::AffectType::Both => test(caster) && is_player(world, target) && test(target),
            })
        }
        // ---- the pre-G34 hold-out -------------------------------------------
        skill::SkillCondition::ExistNpc(c) => {
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
        .get_component::<components::ZoneFlags>(&object_id)
        .is_some_and(|f| f.contains(kind))
}

fn charges(world: &World, object_id: i32) -> i32 {
    helpers::player(world, object_id).map_or(0, |p| p.charges)
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

fn vital_percent(world: &World, object_id: i32, vital: skill::Vital) -> Option<i32> {
    let v = world
        .objects
        .get_component::<components::Vitals>(&object_id)?;
    let (cur, max) = match vital {
        skill::Vital::Hp => (v.cur_hp, v.max_hp as f64),
        skill::Vital::Mp => (v.cur_mp, v.max_mp as f64),
        // CP is the player-only vitals extension, so an NPC has no CP
        // percentage at all — Java's `getCurrentCpPercent` is on `Playable`.
        skill::Vital::Cp => {
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
    crate::game_loop::helpers::player_race(world, object_id)
}

fn target_in_my_party(world: &World, caster: i32, target: i32, include_me: bool) -> bool {
    if !is_player(world, target) {
        return false;
    }
    let party_of = |oid: i32| {
        world
            .objects
            .get_component::<components::PartyRef>(&oid)
            .map(|p| p.0)
    };
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
        && helpers::is_dead(world, target)
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
    let Some(p) = helpers::player(world, caster) else {
        return Err(Refusal(None));
    };
    // `isAlikeDead() || isCursedWeaponEquipped()` — silent.
    if helpers::is_dead(world, caster) || p.cursed_weapon_equipped_id != 0 {
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
    helpers::player(world, caster).is_some_and(|p| {
        !p.is_mounted()
            && !p.teleporting
            && !world
                .objects
                .has_component::<components::OlympiadObserver>(&caster)
    })
}

/// `CanSummonSiegeGolemSkillCondition` / `BuildCampSkillCondition` — the same
/// gate twice in Java: alive, uncursed, in a clan, standing in a residence
/// whose siege is in progress, and on the attacker side of it.
fn siege_deployable(world: &World, caster: i32) -> bool {
    let Some(p) = helpers::player(world, caster) else {
        return false;
    };
    if helpers::is_dead(world, caster) || p.cursed_weapon_equipped_id != 0 || p.clan_id == 0 {
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

/// `CanUntransformSkillCondition` — may this caster drop their transform?
///
/// Two of Java's three legs refuse silently (dead, or holding a cursed weapon).
/// The third is the one the `LandingZone` kind exists for: a wyvern rider has
/// to be low enough, and the 69 landing zones are where "low enough" is.
fn can_untransform(world: &World, caster: i32) -> Result<(), Refusal> {
    let Some(p) = helpers::player(world, caster) else {
        return Err(Refusal(None));
    };
    // `isAlikeDead() || isCursedWeaponEquipped()`.
    if helpers::is_dead(world, caster)
        || flags_of(world, caster) & crate::model::skill::effect_flag::FAKE_DEATH != 0
        || p.cursed_weapon_equipped_id != 0
    {
        return Err(Refusal(None));
    }
    // `isFlyingMounted()` — the wyvern, the one flying mount on this chronicle.
    const MOUNT_WYVERN: u8 = 2;
    if p.mount_type == MOUNT_WYVERN {
        let over_landing = maybe_position(world, caster)
            .is_some_and(|pos| world.data.zone_data.in_landing_zone(pos.x, pos.y, pos.z));
        if !over_landing {
            return Err(Refusal(Some(RefusalLine::Sm(
                sm_ids::YOU_ARE_TOO_HIGH_TO_PERFORM_THIS_ACTION,
            ))));
        }
    }
    Ok(())
}

/// `OpCallPcSkillCondition` — Summon Friend's caster-side gate.
fn call_pc(world: &World, caster: i32) -> Result<(), Refusal> {
    let Some(p) = helpers::player(world, caster) else {
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
    if world
        .objects
        .has_component::<components::OlympiadObserver>(&caster)
    {
        return Err(Refusal(None));
    }
    // `isInsideZone(NO_SUMMON_FRIEND) || isInsideZone(JAIL) || isFlyingMounted()`
    // — plus the jail *punishment* state, which the port tracks separately from
    // the zone and which Java reaches through `PunishmentAffect` instead.
    let in_blocked_zone = maybe_position(world, caster).is_some_and(|pos| {
        world
            .data
            .zone_data
            .in_no_summon_friend_zone(pos.x, pos.y, pos.z)
            || world.data.zone_data.in_jail_zone(pos.x, pos.y, pos.z)
    });
    if p.jailed || in_blocked_zone {
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
    let request_holder = if helpers::player(world, target).is_some() {
        target
    } else {
        world
            .objects
            .get_component::<components::ServitorOf>(&target)
            .map(|s| s.owner_object_id)
            .ok_or(Refusal(None))?
    };

    if !helpers::is_dead(world, target) {
        // Java sends `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` with the skill
        // name on the summon leg, but `RefusalLine` has no skill-name form and
        // the player leg refuses silently — so both refuse silently.
        return Err(Refusal(None));
    }
    if flags_of(world, target) & BLOCK_RESURRECTION != 0 {
        return Err(Refusal(Some(RefusalLine::Sm(sm_ids::REJECT_RESURRECTION))));
    }
    if helpers::player(world, request_holder).is_some_and(|p| p.revive_request.is_some()) {
        return Err(Refusal(Some(RefusalLine::Sm(
            sm_ids::RESURRECTION_HAS_ALREADY_BEEN_PROPOSED,
        ))));
    }
    Ok(())
}
