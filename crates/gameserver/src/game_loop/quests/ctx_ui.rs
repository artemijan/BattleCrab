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

    /// `player.getQuestState(name)` of another script, worked through: `f`
    /// runs on that script's context for the same player and NPC. `None`
    /// when no script has the name.
    pub fn with_other_quest<R>(
        &mut self,
        quest_name: &str,
        f: impl FnOnce(&mut QuestCtx<'_>) -> R,
    ) -> Option<R> {
        let script = self.world.quests.by_name(quest_name)?;
        let mut ctx = QuestCtx::new(self.world, self.client_id, self.player, self.npc, script);
        ctx.simulated = self.simulated;
        Some(f(&mut ctx))
    }

    /// `otherPlayer.getQuestState(name)`, worked through: `f` runs on that
    /// script's context for `player` (with no NPC). `None` when no script has
    /// the name.
    pub fn with_player_quest<R>(
        &mut self,
        player: i32,
        quest_name: &str,
        f: impl FnOnce(&mut QuestCtx<'_>) -> R,
    ) -> Option<R> {
        let script = self.world.quests.by_name(quest_name)?;
        let client_id =
            crate::game_loop::helpers::client_for_player(self.world, player).unwrap_or(0);
        let mut ctx = QuestCtx::new(self.world, client_id, player, 0, script);
        ctx.simulated = self.simulated;
        Some(f(&mut ctx))
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

    /// `new TutorialShowQuestionMark(markId, markType)`.
    pub fn tutorial_show_question_mark(&mut self, mark_id: i32, mark_type: u8) {
        self.send(server_packets::tutorial_show_question_mark(
            mark_id, mark_type,
        ));
    }

    /// `player.sendPacket(ExShowUsm.X)`: the client plays USM video
    /// `video_id`.
    pub fn show_usm(&mut self, video_id: i32) {
        self.send(server_packets::ex_show_usm(video_id));
    }

    /// `player.sendPacket(new ExChangeToAwakenedClass(classId))`.
    pub fn send_change_to_awakened_class(&mut self, class_id: i32) {
        self.send(server_packets::ex_change_to_awakened_class(class_id));
    }

    /// `player.sendPacket(new ExSendUIEvent(player, hide, countUp,
    /// startTime, endTime, npcString))`, times in seconds.
    pub fn send_ui_event(
        &mut self,
        hide: bool,
        count_up: bool,
        start_time: i32,
        end_time: i32,
        npc_string_id: i32,
    ) {
        self.send(server_packets::ex_send_ui_event(
            self.player,
            hide,
            count_up,
            start_time,
            end_time,
            npc_string_id,
        ));
    }

    /// `player.sendPacket(new NpcHtmlMessage(npc.getObjectId(),
    /// getHtm(player, file)))`: one of the script's files as a plain chat
    /// window, never the quest window its `.htm` name would pick.
    pub fn send_plain_html(&mut self, filename: &str) {
        let html = self.get_htm(filename);
        self.send(server_packets::npc_html_message(self.npc, &html));
    }

    /// `MultisellData.separateAndSend(list_id, player, npc, false)`.
    pub fn show_multisell(&mut self, list_id: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::commerce::multisell::separate_and_send(
            self.world,
            self.client_id,
            self.player,
            Some(self.npc),
            list_id,
            false,
        );
    }

    /// `npc.showChatWindow(player)`: the in-context NPC's own landing page,
    /// for an `onFirstTalk` that falls back to it.
    pub fn show_npc_chat_window(&mut self) {
        if self.simulated {
            return;
        }
        crate::game_loop::combat::target::show_chat_window(self.world, self.client_id, self.npc, 0);
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
        crate::game_loop::combat::target::current(self.world, self.player)
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
