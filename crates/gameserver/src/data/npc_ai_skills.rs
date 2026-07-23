//! Port of the AI skill bucketing at the tail of `NpcData.parse` — the block
//! that fills `NpcTemplate._aiSkillLists` (`Map<AISkillScope, List<Skill>>`).
//!
//! Java classifies each *non-passive* template skill once at load into one or
//! more [`AiSkillScope`]s, and `AttackableAI.thinkAttack` then walks those
//! buckets in priority order (heal → buff → immobilize → mute → short range →
//! long range → general). Bucketing at load rather than per-think matters here
//! too: the AI tick runs over every engaged monster once a second, and this
//! dist attaches ~10k active skills across 4831 NPCs.
//!
//! The classification is a straight transcription of Java's if/else ladder, so
//! the *order* of the branches is load-bearing — a skill matches at most one of
//! the `else if` arms even when it carries several effect types (an attack
//! skill that also stuns buckets as ATTACK, never IMMOBILIZE).

use std::collections::HashMap;

use crate::model::skill::{OperateType, SkillEffect};

use super::npc_data::NpcData;
use super::skill_data::SkillData;

/// Java `enums/AISkillScope`. `RES`, `NEGATIVE` and `SUICIDE` are populated
/// but not yet consumed by the ported think ladder — see the module docs on
/// [`crate::game_loop::npc_cast`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiSkillScope {
    Buff,
    Debuff,
    Negative,
    Attack,
    Immobilize,
    Heal,
    Res,
    /// "Chance of trouble" in Java's naming — mutes/blocks/debuffs worth
    /// interrupting a casting target with.
    Cot,
    Universal,
    LongRange,
    ShortRange,
    General,
    Suicide,
}

/// The skills of one NPC template, bucketed by scope. Values are `(id, level)`
/// pairs resolved against [`SkillData`] at cast time, matching how the rest of
/// the port carries NPC skills.
#[derive(Debug, Clone, Default)]
pub struct NpcAiSkills {
    buckets: HashMap<AiSkillScope, Vec<(i32, i32)>>,
}

impl NpcAiSkills {
    pub fn get(&self, scope: AiSkillScope) -> &[(i32, i32)] {
        self.buckets.get(&scope).map_or(&[], Vec::as_slice)
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    fn push(&mut self, scope: AiSkillScope, skill: (i32, i32)) {
        self.buckets.entry(scope).or_default().push(skill);
    }
}

/// npc id → [`NpcAiSkills`]. Templates whose skills are all passive (the
/// overwhelming majority — the 4408/4410/4412 stat holders) get no entry.
#[derive(Debug, Clone, Default)]
pub struct NpcAiSkillIndex {
    by_npc: HashMap<i32, NpcAiSkills>,
}

impl NpcAiSkillIndex {
    pub fn build(npc_data: &NpcData, skill_data: &SkillData) -> Self {
        let mut by_npc = HashMap::new();
        for template in npc_data.all() {
            let mut ai = NpcAiSkills::default();
            for &(skill_id, level) in &template.skill_list {
                let Some(skill) = skill_data.get(skill_id, level) else {
                    continue;
                };
                if skill.operate_type == OperateType::Passive {
                    continue;
                }
                for scope in classify(skill) {
                    ai.push(scope, (skill_id, level));
                }
            }
            if !ai.is_empty() {
                by_npc.insert(template.id, ai);
            }
        }
        Self { by_npc }
    }

    pub fn get(&self, npc_id: i32) -> Option<&NpcAiSkills> {
        self.by_npc.get(&npc_id)
    }

    /// Number of templates with at least one AI-usable skill (for the boot log
    /// and the coverage test).
    pub fn len(&self) -> usize {
        self.by_npc.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_npc.is_empty()
    }
}

/// The `else if` ladder from `NpcData.parse`. Returns every scope the skill
/// lands in; the first matching arm wins, so ordering must not be rearranged.
fn classify(skill: &crate::model::skill::Skill) -> Vec<AiSkillScope> {
    use AiSkillScope as S;

    // `isSuicideAttack` is a skill flag Java reads from the datapack; no skill
    // in this dist declares it (self-destruct mobs are script-driven here), so
    // the branch is ported for shape and never taken. TODO(G21): wire the flag
    // through `SkillData` if a later dist adds it.
    let mut scopes = vec![S::General];

    // <=150 is Java's literal cut between the short- and long-range buckets.
    let range_scope = if skill.cast_range <= 150 {
        S::ShortRange
    } else {
        S::LongRange
    };

    let has =
        |e: &[fn(&SkillEffect) -> bool]| skill.effects.iter().any(|se| e.iter().any(|f| f(se)));

    if skill.is_continuous {
        if !skill.is_debuff {
            scopes.push(S::Buff);
        } else {
            scopes.push(S::Debuff);
            scopes.push(S::Cot);
            scopes.push(range_scope);
        }
    } else if has(&[is_dispel]) {
        scopes.push(S::Negative);
        scopes.push(range_scope);
    } else if has(&[is_heal]) {
        scopes.push(S::Heal);
    } else if has(&[is_attack]) {
        scopes.push(S::Attack);
        scopes.push(S::Universal);
        scopes.push(range_scope);
    } else if has(&[is_sleep]) {
        // Java splits SLEEP (no range scope) from BLOCK_ACTIONS/ROOT (with
        // one). The port has no separate Sleep effect — stun and sleep both
        // parse to `BlockActions` — so the two arms collapse into the
        // BLOCK_ACTIONS/ROOT shape below and this one is unreachable.
        scopes.push(S::Immobilize);
    } else if has(&[is_block_actions, is_root]) {
        scopes.push(S::Immobilize);
        scopes.push(range_scope);
    } else if has(&[is_mute, is_block_control]) {
        scopes.push(S::Cot);
        scopes.push(range_scope);
    } else if has(&[is_dot]) {
        scopes.push(range_scope);
    } else if has(&[is_resurrection]) {
        scopes.push(S::Res);
    } else {
        scopes.push(S::Universal);
    }

    scopes
}

fn is_dispel(e: &SkillEffect) -> bool {
    matches!(
        e,
        SkillEffect::DispelBySlot { .. } | SkillEffect::DispelBySlotProbability { .. }
    )
}
fn is_heal(e: &SkillEffect) -> bool {
    matches!(
        e,
        SkillEffect::Heal { .. } | SkillEffect::HealOverTime { .. }
    )
}
fn is_attack(e: &SkillEffect) -> bool {
    matches!(
        e,
        SkillEffect::PhysicalAttack { .. }
            | SkillEffect::MagicalAttack { .. }
            | SkillEffect::HpDrain { .. }
            | SkillEffect::Blow { .. }
            | SkillEffect::VampiricAttack
    )
}
fn is_sleep(_e: &SkillEffect) -> bool {
    false
}
fn is_block_actions(e: &SkillEffect) -> bool {
    matches!(e, SkillEffect::BlockActions { .. })
}
fn is_root(e: &SkillEffect) -> bool {
    matches!(e, SkillEffect::Root)
}
fn is_mute(e: &SkillEffect) -> bool {
    matches!(e, SkillEffect::Mute | SkillEffect::PhysicalMute)
}
fn is_block_control(e: &SkillEffect) -> bool {
    matches!(e, SkillEffect::BlockControl)
}
fn is_dot(e: &SkillEffect) -> bool {
    matches!(
        e,
        SkillEffect::DamOverTime { .. } | SkillEffect::ManaDamOverTime { .. }
    )
}
fn is_resurrection(_e: &SkillEffect) -> bool {
    // No Resurrection effect is ported yet; the RES bucket stays empty and the
    // AI never revives its dead. TODO(G21): fill in with the effect.
    false
}
