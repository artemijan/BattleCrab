//! `QuestCtx` item and reward primitives (`AbstractScript` ports): give,
//! take, drop-rate rolls, and the quest-flow combinators built on them.

use super::*;

impl<'w> QuestCtx<'w> {
    // --- item / reward primitives (AbstractScript ports) ------------------

    /// `getQuestItemsCount`.
    pub fn quest_items_count(&self, item_id: i32) -> i64 {
        self.world
            .objects
            .get_component::<Inventory>(&self.player)
            .map(|inv| inv.count_of(item_id))
            .unwrap_or(0)
    }

    /// `AbstractScript.giveItems(player, id, count, playSound=false)`.
    pub fn give_items(&mut self, item_id: i32, count: i64) {
        if self.simulated || count <= 0 {
            return;
        }
        // `ItemContainer.addItem` logs `Invalid ItemId` and returns null when
        // the datapack declares no template, so the player receives nothing.
        // The port's shared add path is lenient and would mint the item, which
        // matters whenever a script *checks* for what it just gave: `Q11000`'s
        // Rolento hands over two ids this dist does not define, and it is
        // precisely their absence that strands the quest at cond 8. Validating
        // here keeps quest scripts honest without touching the other give
        // paths (loot, admin, lottery…), whose fixtures rely on the leniency.
        if self.world.data.item_data.get(item_id).is_none() {
            warn!("Invalid ItemId ({item_id}) requested by quest give_items");
            return;
        }
        give_item_with_earned_message(self.world, self.client_id, self.player, item_id, count);
    }

    /// `AbstractScript.rewardItems` — the turn-in variant with the reward
    /// multipliers (`RateQuestRewardAdena` for adena, `RateQuestReward`
    /// otherwise; the per-EtcItem-type multiplier split is unported).
    pub fn reward_items(&mut self, item_id: i32, count: i64) {
        if self.simulated || count <= 0 {
            return;
        }
        let rate = if item_id == ADENA_ID {
            self.world.cfg.rates.rate_quest_reward_adena
        } else {
            self.world.cfg.rates.rate_quest_reward
        };
        let count = (count as f64 * rate) as i64;
        give_item_with_earned_message(self.world, self.client_id, self.player, item_id, count);
    }

    /// `Config.RATE_QUEST_DROP` — the drop-rate multiplier some quests fold
    /// into their own roll threshold (rather than through `give_item_randomly`).
    pub fn rate_quest_drop(&self) -> f64 {
        self.world.cfg.rates.rate_quest_drop
    }

    /// `AbstractScript.giveAdena`.
    pub fn give_adena(&mut self, count: i64, apply_rates: bool) {
        if apply_rates {
            self.reward_items(ADENA_ID, count);
        } else {
            self.give_items(ADENA_ID, count);
        }
    }

    /// The champion arm of `AbstractScript.giveItemRandomly` as a
    /// `(chance multiplier, amount multiplier)` pair — `(1.0, 1.0)` whenever
    /// the notifying NPC is absent, is not a champion, or the master gate is
    /// off. Adena and ancient adena take the `ADENAS_` pair, everything else
    /// the plain one; Java splits them because a 10× adena rate on a normal
    /// item would dwarf the intended reward.
    fn champion_quest_drop_mods(&self, item_id: i32) -> (f64, f64) {
        const ANCIENT_ADENA_ID: i32 = 5575;
        let cfg = &self.world.cfg.champion;
        // `npc != null` — `self.npc` is 0 for the script-driven calls that
        // have no NPC (timers, bypass handlers), and the component lookup also
        // fails once the corpse has decayed.
        let is_champion = cfg.enable
            && self
                .world
                .objects
                .get_component::<crate::model::npc::Npc>(&self.npc)
                .is_some_and(|n| n.champion);
        if !is_champion {
            return (1.0, 1.0);
        }
        if item_id == ADENA_ID || item_id == ANCIENT_ADENA_ID {
            (cfg.adenas_rewards_chance, cfg.adenas_rewards_amount)
        } else {
            (cfg.rewards_chance, cfg.rewards_amount)
        }
    }

    /// `AbstractScript.giveItemRandomly(player, npc, id, amount, limit,
    /// chance, playSound)`: chance and amount ×`RateQuestDrop`, capped at
    /// `limit`; returns true when the limit is (already) reached — the
    /// "collection finished" signal quests key `setCond` off.
    ///
    /// A champion kill multiplies both on top of the quest rate, exactly as
    /// the death-drop path does — Java repeats the whole champion arm here
    /// because quest items never pass through `NpcTemplate.calculateDrops`.
    /// Without it a champion was a pure penalty on a collection quest: ten
    /// times the HP for the same drop rate.
    pub fn give_item_randomly(
        &mut self,
        item_id: i32,
        amount: i64,
        limit: i64,
        chance: f64,
        play_sound: bool,
    ) -> bool {
        if self.simulated {
            return false;
        }
        let current = self.quest_items_count(item_id);
        if limit > 0 && current >= limit {
            return true;
        }
        let rate = self.world.cfg.rates.rate_quest_drop;
        // Java truncates to `long` *before* the champion multiply and again
        // after (`long *= double` is a narrowing compound assignment), so the
        // two casts below are both load-bearing for byte-parity on the amount.
        let mut amount_to_give = (amount as f64 * rate) as i64;
        let mut chance_with_bonus = chance * rate;
        // `(npc != null) && Config.CHAMPION_ENABLE && npc.isChampion()`.
        let (champ_chance, champ_amount) = self.champion_quest_drop_mods(item_id);
        chance_with_bonus *= champ_chance;
        amount_to_give = (amount_to_give as f64 * champ_amount) as i64;
        let random = self.world.roll_f64();
        if chance_with_bonus >= random && amount_to_give > 0 {
            if limit > 0 && current + amount_to_give > limit {
                amount_to_give = limit - current;
            }
            give_item_with_earned_message(
                self.world,
                self.client_id,
                self.player,
                item_id,
                amount_to_give,
            );
            if current + amount_to_give == limit {
                if play_sound {
                    self.play_sound(quest_sounds::MIDDLE);
                }
                return true;
            }
            if play_sound {
                self.play_sound(quest_sounds::ITEMGET);
            }
            return limit <= 0;
        }
        false
    }

    /// `AbstractScript.takeItems` (negative count = all). Returns whether
    /// anything was taken.
    pub fn take_items(&mut self, item_id: i32, count: i64) -> bool {
        if self.simulated {
            return false;
        }
        take_items(self.world, self.client_id, self.player, item_id, count)
    }

    /// `AbstractScript.addExpAndSp` — quest XP/SP with the
    /// `RateQuestRewardXP/SP` multipliers.
    pub fn add_exp_and_sp(&mut self, exp: i64, sp: i64) {
        if self.simulated {
            return;
        }
        add_quest_exp_and_sp(self.world, self.player, exp, sp);
    }

    // --- misc --------------------------------------------------------------

    pub fn play_sound(&mut self, sound: &str) {
        let pkt = server_packets::play_sound(sound);
        self.send(pkt);
    }

    /// `player.sendPacket(new SocialAction(player.getObjectId(), id))` — the
    /// victory animation the class-path quests play on completion. Java uses
    /// `sendPacket`, not a broadcast, so only the player sees it.
    pub fn social_action(&mut self, action_id: i32) {
        let pkt = server_packets::social_action(self.player, action_id);
        self.send(pkt);
    }

    /// `Rnd.get(bound)` through the world RNG (test-forceable).
    pub fn roll(&self, bound: i32) -> i32 {
        self.world.roll(bound)
    }

    pub fn player_level(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .map(|p| p.level)
            .unwrap_or(0)
    }

    /// Hand over one `item_id` and advance to `next_cond` once `target` of them
    /// have been collected — the body of every "kill things until you have N"
    /// quest's `onKill`.
    ///
    /// The sound is the tell that this is one pattern rather than a dozen
    /// similar ones: `ITEMGET` plays on every drop **except** the last, because
    /// [`set_cond`](Self::set_cond) plays `MIDDLE` itself and Java never stacks
    /// the two. A hand-written copy that plays `ITEMGET` unconditionally is
    /// audibly wrong at exactly one moment per quest, which is precisely the
    /// kind of thing that survives review.
    ///
    /// The `==` is Java's, not a `>=`: an inventory already over the target
    /// (a second quest that shares the item, a GM `//give`) does not advance
    /// the condition here.
    pub fn collect_toward(&mut self, item_id: i32, target: i64, next_cond: i32) {
        self.give_items(item_id, 1);
        if self.quest_items_count(item_id) == target {
            self.set_cond(next_cond, true);
        } else {
            self.play_sound(quest_sounds::ITEMGET);
        }
    }

    /// [`collect_toward`](Self::collect_toward) behind the guard every farming
    /// `onKill` opens with: a live state sitting on exactly `cond`. The whole
    /// body of the Pet Ticket trio's (42/43/44) `onKill`, where the item id is
    /// the only thing that differs between them.
    pub fn collect_toward_on_cond(&mut self, cond: i32, item_id: i32, target: i64, next_cond: i32) {
        if !self.has_qs() || !self.is_cond(cond) {
            return;
        }
        self.collect_toward(item_id, target, next_cond);
    }

    /// [`collect_toward`](Self::collect_toward) with a cap: nothing is handed
    /// over once the player already holds `target`, and the condition advances
    /// on `>=` rather than `==`.
    ///
    /// The two differences travel together, which is why this is a separate
    /// method rather than a flag. Java writes these quests with the cap test in
    /// the `onKill` guard itself; that makes an exact `==` the wrong
    /// comparison, because a player who arrived over the target — a shared drop
    /// from another quest — would never see the count *land* on it.
    pub fn collect_capped(&mut self, item_id: i32, target: i64, next_cond: i32) {
        if self.quest_items_count(item_id) >= target {
            return;
        }
        self.give_items(item_id, 1);
        if self.quest_items_count(item_id) >= target {
            self.set_cond(next_cond, true);
        } else {
            self.play_sound(quest_sounds::ITEMGET);
        }
    }

    /// The one-shot drop the trial/testimony chains hang their set-collection
    /// legs off: `if (!hasQuestItems(player, item)) { giveItems(item, 1);
    /// playSound(MIDDLE); }`. A single copy is ever owed, so a second kill of
    /// the same named monster hands out nothing and stays silent.
    ///
    /// The sound is `MIDDLE`, not `ITEMGET`, precisely because there is no
    /// count to tick towards — every drop of a one-shot item *is* the
    /// milestone. Returns whether the item was newly awarded, which is what the
    /// set legs test before advancing their condition: a re-kill must not
    /// re-fire `setCond`.
    pub fn award_once(&mut self, item_id: i32) -> bool {
        if self.quest_items_count(item_id) > 0 {
            return false;
        }
        self.give_items(item_id, 1);
        self.play_sound(quest_sounds::MIDDLE);
        true
    }

    /// The stock hand-in: give up one quest item for the next one and advance
    /// the condition, but only if the player is actually holding it.
    ///
    /// Returns whether the swap happened — the page named by the bypass is the
    /// answer only then, which is Java's "fall through to `null`" for a player
    /// who clicked a link for an item they no longer have.
    pub fn swap_quest_item(&mut self, take: i32, give: i32, next_cond: i32) -> bool {
        if self.quest_items_count(take) == 0 {
            return false;
        }
        self.take_items(take, 1);
        self.give_items(give, 1);
        self.set_cond(next_cond, true);
        true
    }

    /// The trial/testimony `"ACCEPT"` body: start the quest and hand over its
    /// starting letter/sigil, with `MIDDLE` on top of the `ACCEPT` sound
    /// [`start_quest`](Self::start_quest) already played — both sounds, in that
    /// order, as Java's accept blocks do.
    ///
    /// The "only if they don't already hold it" guard is Java's own: an item
    /// left over from an earlier run of the quest is not doubled.
    ///
    /// Returns whether the quest was CREATED, i.e. whether anything happened —
    /// the accept page is only the answer when it was.
    pub fn accept_with_item(&mut self, item_id: i32) -> bool {
        if !self.is_created() {
            return false;
        }
        self.start_quest();
        if self.quest_items_count(item_id) == 0 {
            self.give_items(item_id, 1);
        }
        self.play_sound(quest_sounds::MIDDLE);
        true
    }

    /// Give one of whatever a `(npc_id, item_id)` table yields for the NPC that
    /// just died, then play the pickup sound.
    ///
    /// `fallback` is Java's `else` branch: these quests register more kill
    /// targets than they tabulate drops for, and every untabled one yields the
    /// quest's staple item.
    pub fn give_table_drop(&mut self, table: &[(i32, i32)], fallback: i32) {
        let item = table
            .iter()
            .find(|(id, _)| *id == self.npc_id)
            .map_or(fallback, |(_, item)| *item);
        self.give_items(item, 1);
        self.play_sound(quest_sounds::ITEMGET);
    }

    /// Java `addCondLevel(min, max, html)` — a two-sided level gate, shaped as
    /// the `Some(html)` a `start_condition_html` returns when the gate refuses.
    ///
    /// Both bounds are inclusive, matching Java.
    pub fn cond_level(&self, min: i32, max: i32, html: &str) -> Option<String> {
        let level = self.player_level();
        (level < min || level > max).then(|| html.to_string())
    }
}
