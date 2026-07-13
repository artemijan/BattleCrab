//! Skill handlers: the casting pipeline (`cast`), effect application
//! (`effects`), and skill acquisition (inline here).

pub(crate) mod cast;
pub(crate) mod effects;

use crate::db;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

/// Port of `clientpackets/RequestAcquireSkill.runImpl`, `AcquireSkillType::CLASS`
/// only (see the G6 plan's scope notes — every other type is silently
/// ignored, same as Java ignores an out-of-state/unsupported request).
pub(crate) fn handle_request_acquire_skill(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestAcquireSkill::read(body) else { return };
    if pkt.acquire_type != cp::RequestAcquireSkill::CLASS {
        return;
    }
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    let Some(player) = world.objects.get_component::<crate::model::Player>(&object_id) else { return };
    let Some(learn) = world.data.skill_trees.skill_learn(player.class_id, pkt.skill_id, pkt.skill_level) else { return };
    if learn.get_level > player.level || learn.level_up_sp > player.sp {
        return; // TODO: SystemMessage (level/SP gate)
    }
    let (skill_id, skill_level, level_up_sp) = (learn.skill_id, learn.skill_level, learn.level_up_sp);

    if let Some(player) = world.objects.get_component_mut::<crate::model::Player>(&object_id) {
        player.sp -= level_up_sp;
    }
    if let Some(book) = world.objects.get_component_mut::<crate::model::components::SkillBook>(&object_id) {
        book.0.insert(skill_id, skill_level);
    }
    let _ = world.db.send(db::DbCommand::UpsertSkill { char_id: object_id, skill_id, skill_level });

    if let Some(v) = crate::model::PlayerView::of(&world.objects, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            let Some(skills) = world.objects.get_component::<crate::model::components::SkillBook>(&object_id)
            else {
                return;
            };
            cs.send(server_packets::acquire_skill_done());
            cs.send(crate::network::enter_world::skill_list(skills, &world.data));
            cs.send(crate::network::enter_world::acquire_skill_list(v.p, skills, &world.data));
            cs.send(crate::network::user_info::user_info(&v, &world.data, &world.cfg.character));
        }
    }
    // `player.updateShortCuts(_id, _level, 0)` — refresh SKILL slots holding
    // the upgraded skill.
    super::shortcuts::update_skill_shortcuts(world, object_id, skill_id, skill_level);
}

