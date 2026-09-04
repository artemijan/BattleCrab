//! `QuestCtx` tutorial-window and screen helpers (Q255), plus
//! ground-item drops near the NPC.

use super::QuestCtx;
use super::load_quest_html;
use crate::model::components::social::Quests;
use crate::model::quest;
use crate::network::server_packets;
impl<'w> QuestCtx<'w> {
    // --- Tutorial window / global-event helpers (Q255) ---------------------

    /// `QuestState.isMemoState`.
    pub fn is_memo_state(&self, value: i32) -> bool {
        self.memo_state() == value
    }

    /// Another quest's `getMemoState` (with Java's STARTED gate).
    pub fn other_quest_memo_state(&self, quest_name: &str) -> i32 {
        self.world
            .objects
            .get_component::<Quests>(&self.player)
            .and_then(|q| q.0.get(quest_name))
            .map(|qs| {
                if qs.is_started() {
                    qs.get_int(quest::MEMO_VAR)
                } else {
                    0
                }
            })
            .unwrap_or(0)
    }

    /// Whether the player has a quest state for another quest at all (Java
    /// `player.getQuestState(name) != null`).
    pub fn has_other_quest_state(&self, quest_name: &str) -> bool {
        self.world
            .objects
            .get_component::<Quests>(&self.player)
            .is_some_and(|q| q.0.contains_key(quest_name))
    }

    /// Write a var on *another* quest's state (Java
    /// `getQuestState(name).set(...)` — the NewbieGuide advancing Q255's
    /// memoState). No-op when the player has no state for that quest.
    pub fn set_other_quest_var(&mut self, quest_name: &str, var: &str, value: impl Into<String>) {
        if let Some(qs) = self
            .world
            .objects
            .get_component_mut::<Quests>(&self.player)
            .and_then(|q| q.0.get_mut(quest_name))
        {
            qs.vars.insert(var.to_string(), value.into());
        }
    }

    /// `TutorialShowHtml` with the content of a file from the script's html
    /// dir (Java `showTutorialHtml(getHtm(player, file))`).
    pub fn tutorial_show_html_file(&mut self, filename: &str) {
        let html = load_quest_html(self.world, self.player, &self.script, filename)
            .unwrap_or_else(|| format!("<html><body>File {filename} not found.</body></html>"));
        self.send(server_packets::tutorial_show_html(&html));
    }

    pub fn tutorial_show_question_mark(&mut self, mark_id: i32) {
        self.send(server_packets::tutorial_show_question_mark(mark_id));
    }

    pub fn tutorial_close_html(&mut self) {
        self.send(server_packets::tutorial_close_html());
    }

    /// `playTutorialVoice` — a `PlaySound(2, voice, …)` anchored at the
    /// player's position.
    pub fn play_tutorial_voice(&mut self, voice: &str) {
        let Some(pos) = self
            .world
            .objects
            .get_component::<crate::model::components::space::Position>(&self.player)
            .copied()
        else {
            return;
        };
        self.send(server_packets::play_tutorial_voice(
            voice, pos.x, pos.y, pos.z,
        ));
    }

    /// `ExShowScreenMessage` (the tutorial uses TOP_CENTER = 2).
    pub fn show_screen_message(&mut self, text: &str, position: i32, time_ms: i32) {
        self.send(server_packets::ex_show_screen_message(
            text, position, time_ms,
        ));
    }

    /// The template id of whatever the player currently targets, 0 when
    /// nothing / not an NPC (Java `player.getTarget().getId()`).
    pub fn player_target_npc_id(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::components::combat::TargetRef>(&self.player)
            .and_then(|t| t.0)
            .and_then(|oid| {
                self.world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&oid)
            })
            .map(|n| n.npc_id)
            .unwrap_or(0)
    }

    /// `Npc.dropItem(killer, …)`: toss an item on the ground at the involved
    /// NPC's feet (the tutorial gremlins' Blue Gemstone).
    pub fn drop_item_from_npc(&mut self, item_id: i32, count: i64) {
        let Some(pos) = self
            .world
            .objects
            .get_component::<crate::model::components::space::Position>(&self.npc)
            .copied()
        else {
            return;
        };
        let npc = self.npc;
        crate::game_loop::items::ground_items::spawn_ground_item(
            self.world,
            item_id,
            count,
            0,
            pos.x,
            pos.y,
            pos.z,
            npc,
            crate::game_loop::items::ground_items::DropSource::Npc,
        );
    }

    /// Ground items of `item_id` within `radius` (2D) of the involved NPC
    /// (Java's `World.getVisibleObjectsInRange` gem-count cap).
    pub fn count_ground_items_near_npc(&self, item_id: i32, radius: f64) -> usize {
        let Some(npos) = self
            .world
            .objects
            .get_component::<crate::model::components::space::Position>(&self.npc)
        else {
            return 0;
        };
        self.world
            .ground_item_regions
            .values()
            .flat_map(|v| v.iter())
            .filter(|oid| {
                self.world
                    .objects
                    .get_component::<crate::model::components::commerce::GroundItem>(oid)
                    .is_some_and(|g| g.item_id == item_id)
                    && self
                        .world
                        .objects
                        .get_component::<crate::model::components::space::Position>(oid)
                        .is_some_and(|p| npos.distance_2d(p) <= radius)
            })
            .count()
    }
}
