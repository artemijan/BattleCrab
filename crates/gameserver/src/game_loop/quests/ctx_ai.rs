//! The orders an instance script gives an NPC through its [`QuestCtx`]:
//! follow, move-to, run, heading and `broadcastInfo`, hate and damage
//! between NPCs, death, the display effect, a skill's effects applied from
//! the NPC, and a player's movie.

use super::QuestCtx;
use crate::game_loop::combat::{DamageHit, npc_receive_damage};
use crate::game_loop::helpers;
use crate::game_loop::npc::ai;
use crate::game_loop::space::position::region_cell_of;
use crate::model::components::space::Position;
use crate::model::npc::{AggroList, Npc};

impl QuestCtx<'_> {
    /// `npc.getAI().startFollow(target)`.
    pub fn npc_start_follow(&mut self, npc: i32, target: i32) {
        if self.simulated {
            return;
        }
        ai::follow::start_follow(self.world, npc, target);
    }

    /// `npc.getAI().setIntention(Intention.MOVE_TO, loc)`: refused while the
    /// NPC casts or cannot act, else the swing is aborted and the walk starts.
    pub fn npc_move_to(&mut self, npc: i32, (x, y, z): (i32, i32, i32)) {
        if self.simulated
            || crate::game_loop::abnormal::is_blocked_from_actions(self.world, npc)
            || self
                .world
                .objects
                .has_component::<crate::model::components::combat::Casting>(&npc)
        {
            return;
        }
        crate::game_loop::combat::abort_attack(self.world, npc);
        ai::set_move_to_intention(self.world, npc, x, y, z);
    }

    /// `npc.setRunning()`.
    pub fn npc_set_running(&mut self, npc: i32) {
        if self.simulated {
            return;
        }
        ai::set_running(self.world, npc);
    }

    /// `Quest.addAttackDesire(npc, target)`: run, then the ATTACK intention
    /// on `target`. Only a `FriendlyNpc` takes the order; the monster AI
    /// reads its victim from the hate list, which callers seed instead.
    pub fn npc_attack_desire(&mut self, npc: i32, target: i32) {
        if self.simulated {
            return;
        }
        ai::set_running(self.world, npc);
        if self.npc_is_friendly(npc) {
            ai::friendly::set_attack_intention(self.world, npc, target);
        }
    }

    /// `npc.teleToLocation(new Location(x, y, z))`, heading 0.
    pub fn npc_teleport(&mut self, npc: i32, (x, y, z): (i32, i32, i32)) {
        if self.simulated {
            return;
        }
        crate::game_loop::npc::relocate_npc(self.world, npc, x, y, z, 0);
    }

    /// `npc.getHeading()`.
    pub fn npc_heading(&self, npc: i32) -> i32 {
        self.world
            .objects
            .get_component::<Position>(&npc)
            .map_or(0, |p| p.heading)
    }

    /// `npc.setHeading(heading)` — server side only; `broadcastInfo` shows it.
    pub fn npc_set_heading(&mut self, npc: i32, heading: i32) {
        if self.simulated {
            return;
        }
        if let Some(p) = self.world.objects.get_component_mut::<Position>(&npc) {
            p.heading = heading;
        }
    }

    /// `npc.broadcastInfo()` — a fresh `NpcInfo` to everyone who sees it.
    pub fn npc_broadcast_info(&mut self, npc: i32) {
        if self.simulated {
            return;
        }
        let (Some(region), Some(pkt)) = (
            region_cell_of(self.world, npc),
            crate::game_loop::space::visibility::npc_info_bytes(self.world, npc),
        ) else {
            return;
        };
        let instance = helpers::instance_of(self.world, npc);
        crate::game_loop::net::broadcast::broadcast_near_region_in(
            self.world, region, instance, &pkt,
        );
    }

    /// `npc.abortAttack()`.
    pub fn npc_abort_attack(&mut self, npc: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::combat::abort_attack(self.world, npc);
    }

    /// `npc.asAttackable().addDamageHate(attacker, damage, hate)`. A
    /// `FriendlyNpc` takes none from a playable or another friendly; a zero
    /// hate from a non-player still counts as 1.
    pub fn npc_add_damage_hate(&mut self, npc: i32, attacker: i32, damage: f64, hate: f64) {
        if self.simulated || npc == attacker {
            return;
        }
        let friendly = |oid: i32| {
            self.world
                .objects
                .get_component::<Npc>(&oid)
                .is_some_and(|n| n.friendly_ai(self.world))
        };
        if friendly(npc) && (helpers::is_playable(self.world, attacker) || friendly(attacker)) {
            return;
        }
        let is_player = self
            .world
            .objects
            .has_component::<crate::model::Player>(&attacker);
        if let Some(aggro) = self.world.objects.get_component_mut::<AggroList>(&npc) {
            let entry = aggro.0.entry(attacker).or_default();
            entry.damage += damage;
            entry.hate += if hate == 0.0 && !is_player { 1.0 } else { hate };
        }
    }

    /// `npc.reduceCurrentHp(amount, attacker, null)`.
    pub fn npc_reduce_current_hp(&mut self, npc: i32, amount: f64, attacker: i32) {
        if self.simulated {
            return;
        }
        npc_receive_damage(self.world, npc, attacker, amount, DamageHit::default());
    }

    /// `npc.doDie(killer)`.
    pub fn npc_do_die(&mut self, npc: i32, killer: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::npc::npc_do_die(self.world, npc, killer);
    }

    /// `npc.setDisplayEffect(state)`.
    pub fn npc_set_display_effect(&mut self, npc: i32, state: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::admin::effects::set_display_effect(self.world, npc, state);
    }

    /// `new SkillHolder(id, level).getSkill().applyEffects(effector, effected)`.
    pub fn apply_skill_effects(&mut self, skill_id: i32, level: i32, effector: i32, effected: i32) {
        if self.simulated {
            return;
        }
        let Some(skill) = self.world.data.skill_data.get(skill_id, level).cloned() else {
            return;
        };
        crate::game_loop::skills::effects::apply_skill_effects(
            self.world, effector, effected, &skill,
        );
    }

    /// `playMovie(player, movie)`; `movie_id` is the `Movie` client id.
    pub fn play_movie(&mut self, player: i32, movie_id: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::admin::effects::play_movie(self.world, player, movie_id);
    }
}
