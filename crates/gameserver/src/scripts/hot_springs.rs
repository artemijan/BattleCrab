//! `ai/areas/HotSprings` — the spring monsters infect their attackers with
//! diseases (Rheumatism, Cholera, Flu, Malaria). Each 10% proc casts the
//! disease at one level above what the victim already carries, up to 10 —
//! the reason a long Hot Springs session ends with a debuff bar full of
//! level-10 plagues.

use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::quests::{QuestCtx, QuestScript};

const BANDERSNATCHLING: i32 = 21314;
const FLAVA: i32 = 21316;
const ATROXSPAWN: i32 = 21317;
const NEPENTHES: i32 = 21319;
const ATROX: i32 = 21321;
const BANDERSNATCH: i32 = 21322;

const RHEUMATISM: i32 = 4551;
const CHOLERA: i32 = 4552;
const FLU: i32 = 4553;
const MALARIA: i32 = 4554;

const DISEASE_CHANCE: i32 = 10;

pub struct HotSprings;

/// Java `tryToInfect`: the next level is the victim's current level + 1
/// (capped at 10), fresh victims start at 1.
fn infect(ctx: &mut QuestCtx, disease_id: i32) {
    let current = ctx
        .world
        .objects
        .get_component::<crate::model::components::Buffs>(&ctx.player)
        .and_then(|b| {
            b.0.iter()
                .find(|x| x.skill_id == disease_id)
                .map(|x| x.skill_level)
        });
    let level = match current {
        None => 1,
        Some(l) if l < 10 => l + 1,
        Some(_) => 10,
    };
    let Some(skill) = skill_by_id(ctx.world, disease_id, level) else {
        return;
    };
    let npc = ctx.npc;
    let player = ctx.player;
    if crate::game_loop::npc::cast::check_use_conditions_pub(ctx.world, npc, &skill) {
        crate::game_loop::npc::cast::start_cast(ctx.world, npc, player, &skill);
    }
}

impl QuestScript for HotSprings {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "HotSprings"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/HotSprings"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[
            BANDERSNATCHLING,
            FLAVA,
            ATROXSPAWN,
            NEPENTHES,
            ATROX,
            BANDERSNATCH,
        ]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if ctx.roll(100) < DISEASE_CHANCE {
            infect(ctx, MALARIA);
        }
        if ctx.roll(100) < DISEASE_CHANCE {
            let disease = match ctx.npc_id {
                BANDERSNATCHLING | ATROX => RHEUMATISM,
                FLAVA | NEPENTHES => CHOLERA,
                ATROXSPAWN | BANDERSNATCH => FLU,
                _ => return,
            };
            infect(ctx, disease);
        }
    }
}
