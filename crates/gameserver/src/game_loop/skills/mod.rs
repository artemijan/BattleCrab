//! Skill handlers: the casting pipeline (`cast`), effect application
//! (`effects`), and skill acquisition (inline here).

pub(crate) mod affect;
pub(crate) mod cast;
pub(crate) mod effects;

use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

/// Port of `clientpackets/RequestDispel.runImpl` — the client's alt+click
/// buff-cancel on a buff icon (ex-opcode `0xD0:0x0048`). Strips one buff off the
/// player (forced removal, reverting its stats and refreshing the icons), gated
/// exactly as Java gates it.
pub(crate) fn handle_request_dispel(world: &mut World, client_id: u32, ex_body: &[u8]) {
    let Some(pkt) = cp::RequestDispel::read(ex_body) else {
        return;
    };
    // Java: `(_skillId <= 0) || (_skillLevel <= 0)` → return.
    if pkt.skill_id <= 0 || pkt.skill_level <= 0 {
        return;
    }
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    // Java looks up `(skillId, skillLevel, skillSubLevel)`; the Rust skill store
    // is keyed by `(id, level)` only (sub-level is always 0 in Interlude), so the
    // sub-level is validated in the reader but not used for the lookup.
    let (can_be_dispelled, is_debuff, is_transform, is_dance, skill_id) =
        match world.data.skill_data.get(pkt.skill_id, pkt.skill_level) {
            Some(skill) => (
                skill.can_be_dispelled,
                skill.is_debuff,
                skill.abnormal_type == "TRANSFORM",
                skill.magic_type == 3, // Java `Skill.isDance()`
                skill.id,
            ),
            None => return,
        };
    // Java: `!skill.canBeDispelled() || skill.isDebuff()` → return, then the
    // TRANSFORM abnormal-type guard, then the dance gate under `DanceCancelBuff`.
    if !can_be_dispelled || is_debuff || is_transform {
        return;
    }
    if is_dance && !world.cfg.character.dance_cancel_buff {
        return;
    }
    // Java: `player.getObjectId() == _objectId` → `stopSkillEffects(REMOVED, id)`.
    // `handle_buff_expire` is the forced-removal path (reverts stats, rebroadcasts
    // UserInfo + AbnormalStatusUpdate) — same end state as `stopSkillEffects`.
    if pkt.object_id == object_id {
        effects::handle_buff_expire(world, object_id, skill_id);
    } else if crate::game_loop::servitor::pet_of(world, object_id) == Some(pkt.object_id)
        || crate::game_loop::servitor::servitor_of(world, object_id) == Some(pkt.object_id)
    {
        // Java's else branch: the alt-clicked object is the player's pet
        // (`getPet()`) or servitor (`getServitor(_objectId)`) — strip it there.
        effects::handle_buff_expire(world, pkt.object_id, skill_id);
    }
}

/// Port of `clientpackets/RequestAcquireSkill.runImpl`, `AcquireSkillType::CLASS`
/// only (see the G6 plan's scope notes — every other type is silently
/// ignored, same as Java ignores an out-of-state/unsupported request).
pub(crate) fn handle_request_acquire_skill(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestAcquireSkill::read(body) else {
        return;
    };
    if pkt.acquire_type == cp::RequestAcquireSkill::PLEDGE {
        // The rep-gated clan-skill learn (G18) lives with the clan handlers.
        super::clans::handle_learn_pledge_skill(world, client_id, pkt.skill_id, pkt.skill_level);
        return;
    }
    if pkt.acquire_type != cp::RequestAcquireSkill::CLASS {
        return;
    }
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();

    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };
    let Some(learn) =
        world
            .data
            .skill_trees
            .skill_learn(player.class_id, pkt.skill_id, pkt.skill_level)
    else {
        return;
    };
    // `RequestAcquireSkill.checkPlayerSkill` gates, in Java's order: minimum
    // level first, then SP (the SP gate only bites a skill that actually costs
    // SP). Each failure sends its own SystemMessage and stops.
    if learn.get_level > player.level {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                server_packets::sm_ids::YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS,
                &[],
            ));
        }
        return;
    }
    if learn.level_up_sp > 0 && learn.level_up_sp > player.sp {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL,
                &[],
            ));
        }
        return;
    }
    // TODO(G6): 2nd/3rd-class trees add book-gated skills (`learn.requires_item`,
    // e.g. Divine Inspiration). Java's `checkPlayerSkill` also verifies and
    // consumes `getRequiredItems` here (ITEM_MISSING_TO_LEARN_SKILL on failure);
    // we don't parse the item id/count yet, so such skills are learned for free.
    let (skill_id, skill_level, level_up_sp) =
        (learn.skill_id, learn.skill_level, learn.level_up_sp);

    if let Some(player) = world
        .objects
        .get_component_mut::<crate::model::Player>(&object_id)
    {
        player.sp -= level_up_sp;
    }
    if let Some(book) = world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&object_id)
    {
        // Memory-first: the learned skill (and the SP spend above) live in memory
        // and persist on the next flush.
        book.0.insert(skill_id, skill_level);
    }
    // A newly learned passive (e.g. Shield Mastery, Expand Inventory) must
    // contribute its `StatModifier` immediately, not just at the next login —
    // `recompute_conditioned_passives` already diffs "book vs currently-applied
    // passive buffs" generically (it isn't armor-condition-specific despite the
    // module name), so this reuses it rather than a bespoke re-apply.
    super::passive_skills::recompute_conditioned_passives(world, object_id);

    if let Some(v) = crate::model::PlayerView::of(&world.objects, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            let Some(skills) = world
                .objects
                .get_component::<crate::model::components::SkillBook>(&object_id)
            else {
                return;
            };
            cs.send(server_packets::acquire_skill_done());
            if let Some(pkt) = super::helpers::skill_list_packet(world, object_id) {
                cs.send(pkt);
            }
            cs.send(crate::network::enter_world::acquire_skill_list(
                v.p,
                skills,
                &world.data,
            ));
            cs.send(crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                crate::game_loop::party::calculate_relation(world, v.p),
            ));
        }
    }
    // `player.updateShortCuts(_id, _level, 0)` — refresh SKILL slots holding
    // the upgraded skill.
    super::shortcuts::update_skill_shortcuts(world, object_id, skill_id, skill_level);
}
