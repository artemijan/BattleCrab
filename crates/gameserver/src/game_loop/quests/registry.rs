//! The quest registry: script list plus the per-event npc-id indices.

use super::*;
/// The village-master `onEvent` tail: a dialog page of one of `npcs`
/// (`<npcId>-xx.htm`) coming back through a `Quest <name> <page>` bypass,
/// which the script simply echoes for [`show_result`] to render. `None` for
/// anything else, so the caller can fall through to its own handling.
pub fn echoed_page(event: &str, npcs: &[i32]) -> Option<String> {
    let own_page =
        event.ends_with(".htm") && npcs.iter().any(|id| event.starts_with(&id.to_string()));
    own_page.then(|| event.to_string())
}

/// Java `QuestManager` + the per-`NpcTemplate` listener containers, built
/// once at boot: name lookup plus npc-id → script indexes for the
/// start/talk/kill event routes.
pub struct QuestRegistry {
    scripts: Vec<Arc<dyn QuestScript>>,
    by_name: HashMap<&'static str, usize>,
    start: HashMap<i32, Vec<usize>>,
    talk: HashMap<i32, Vec<usize>>,
    kill: HashMap<i32, Vec<usize>>,
    attack: HashMap<i32, Vec<usize>>,
    spawn: HashMap<i32, Vec<usize>>,
    skill_see: HashMap<i32, Vec<usize>>,
    aggro_enter: HashMap<i32, Vec<usize>>,
    spell_finished: HashMap<i32, Vec<usize>>,
    creature_see: HashMap<i32, Vec<usize>>,
    first_talk: HashMap<i32, usize>,
    /// Scripts with `handles_global_events()` (login / tutorial-mark /
    /// item-pickup listeners).
    global_events: Vec<usize>,
}

impl QuestRegistry {
    pub fn new(scripts: Vec<Arc<dyn QuestScript>>) -> Self {
        let mut by_name = HashMap::new();
        let mut start: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut talk: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut kill: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut attack: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut spawn: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut skill_see: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut aggro_enter: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut spell_finished: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut creature_see: HashMap<i32, Vec<usize>> = HashMap::new();
        // One entry per NPC: the first-talk listener owns the whole chat
        // window, so two scripts claiming the same NPC is a bug, not a fan-out.
        let mut first_talk: HashMap<i32, usize> = HashMap::new();
        let mut global_events: Vec<usize> = Vec::new();
        for (idx, s) in scripts.iter().enumerate() {
            by_name.insert(s.name(), idx);
            if s.handles_global_events() {
                global_events.push(idx);
            }
            for &id in s.start_npcs() {
                start.entry(id).or_default().push(idx);
            }
            for &id in s.talk_npcs() {
                talk.entry(id).or_default().push(idx);
            }
            for &id in s.kill_npcs() {
                kill.entry(id).or_default().push(idx);
            }
            for &id in s.attack_npcs() {
                attack.entry(id).or_default().push(idx);
            }
            for &id in s.spawn_npcs() {
                spawn.entry(id).or_default().push(idx);
            }
            for &id in s.skill_see_npcs() {
                skill_see.entry(id).or_default().push(idx);
            }
            for &id in s.aggro_enter_npcs() {
                aggro_enter.entry(id).or_default().push(idx);
            }
            for &id in s.spell_finished_npcs() {
                spell_finished.entry(id).or_default().push(idx);
            }
            for &id in s.creature_see_npcs() {
                creature_see.entry(id).or_default().push(idx);
            }
            for &id in s.first_talk_npcs() {
                if let Some(&prev) = first_talk.get(&id) {
                    warn!(
                        "QuestRegistry: npc {id} first-talk claimed by both [{}] and [{}]; keeping the first.",
                        scripts[prev].name(),
                        s.name(),
                    );
                    continue;
                }
                first_talk.insert(id, idx);
            }
        }
        Self {
            scripts,
            by_name,
            start,
            talk,
            kill,
            attack,
            spawn,
            skill_see,
            aggro_enter,
            creature_see,
            spell_finished,
            first_talk,
            global_events,
        }
    }

    /// Scripts subscribed to the GLOBAL_PLAYERS event stream.
    pub fn global_event_quests(&self) -> Vec<Arc<dyn QuestScript>> {
        self.global_events
            .iter()
            .map(|&i| self.scripts[i].clone())
            .collect()
    }

    /// The script owning `npc_id`'s chat window, if any (Java
    /// `npc.hasListener(ON_NPC_FIRST_TALK)` + the listener itself).
    pub fn first_talk_quest(&self, npc_id: i32) -> Option<Arc<dyn QuestScript>> {
        self.first_talk
            .get(&npc_id)
            .map(|&i| self.scripts[i].clone())
    }

    pub fn by_name(&self, name: &str) -> Option<Arc<dyn QuestScript>> {
        self.by_name.get(name).map(|&i| self.scripts[i].clone())
    }

    /// The scripts that register any hook on `npc_id`, sorted and deduped —
    /// `//show_quests`' listing for an NPC target. Java gets the same set by
    /// walking every `EventType`'s listeners on the spawned NPC and collecting
    /// their owning quests into a `TreeSet` (alphabetical, one entry per
    /// quest however many hooks it registers).
    pub fn scripts_for_npc(&self, npc_id: i32) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = self
            .scripts
            .iter()
            .filter(|s| {
                s.start_npcs().contains(&npc_id)
                    || s.talk_npcs().contains(&npc_id)
                    || s.first_talk_npcs().contains(&npc_id)
                    || s.kill_npcs().contains(&npc_id)
                    || s.attack_npcs().contains(&npc_id)
            })
            .map(|s| s.name())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Every registered script's name, sorted — the `//quest_info` listing.
    pub fn names(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = self.scripts.iter().map(|s| s.name()).collect();
        v.sort_unstable();
        v
    }

    pub fn by_id(&self, quest_id: i32) -> Option<Arc<dyn QuestScript>> {
        self.scripts.iter().find(|s| s.id() == quest_id).cloned()
    }

    pub fn quest_id(&self, name: &str) -> Option<i32> {
        self.by_name.get(name).map(|&i| self.scripts[i].id())
    }

    /// Scripts listing `npc_id` as a talk NPC (the quest-window set).
    pub fn talk_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.talk
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a kill NPC.
    pub fn kill_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.kill
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as an attack NPC.
    pub fn attack_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.attack
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a skill-see NPC.
    pub fn skill_see_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.skill_see
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as an aggro-range-enter NPC.
    pub fn aggro_enter_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.aggro_enter
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a spell-finished NPC.
    pub fn spell_finished_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.spell_finished
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a creature-see NPC.
    pub fn creature_see_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.creature_see
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Whether any script watches `npc_id` for creatures entering sight.
    pub fn has_creature_see(&self, npc_id: i32) -> bool {
        self.creature_see.contains_key(&npc_id)
    }

    /// Scripts listing `npc_id` as a spawn NPC.
    pub fn spawn_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.spawn
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Whether `npc_id` is a start NPC of the named script (the
    /// `ON_NPC_QUEST_START` owner check in `notifyTalk`).
    pub fn is_start_npc(&self, name: &str, npc_id: i32) -> bool {
        self.start
            .get(&npc_id)
            .is_some_and(|v| v.iter().any(|&i| self.scripts[i].name() == name))
    }

    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }
}
