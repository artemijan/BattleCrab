//! Little Wing's Big Adventure (421) — `quests/Q00421_LittleWingsBigAdventure`.
//! The sequel to quest 420: it turns a **hatchling** (summoned from the
//! Dragonflute quest 420 awards) into a grown **strider**. Cronos checks the
//! flute (level ≥ 45, exactly one flute, its enchant level — the hatchling's
//! level — ≥ 55) and sends the player to Mimyu, who binds the rite to *that*
//! flute's object id. Mimyu hands over four **Fairy Leaves**; the player must
//! walk the hatchling to each of the four **Trees of Vision** (Wind / Star /
//! Twilight / Abyss) and have *the pet* (not the player) drink from it — each
//! drink, past a per-tree hit threshold, consumes one leaf and sets that tree's
//! bit in `memoState`. All four bits (`memoState == 15`) → cond 3; Mimyu then
//! trades the flute for the matching **Dragon Bugle** (the strider whistle).
//!
//! Pet infrastructure this exercises (all from G29): [`QuestCtx::
//! pet_control_object_id`] (Java `getPet().getControlObjectId()`, the
//! flute-identity link), [`QuestCtx::attack_is_summon`] (the drink must come
//! from the pet), [`QuestCtx::item_enchant_level`] / [`QuestCtx::item_object_id`]
//! (the flute gates), and the new [`QuestCtx::schedule_despawn`] for the
//! kill-a-tree Guardian ambush.
//!
//! **Dist-data gap (off-chronicle):** this Classic dist ships no hatchling pet
//! template nor a Dragonflute→hatchling summon skill (only `16097_Training_Pet`
//! under `stats/pets/`), so a flute cannot actually summon its hatchling
//! in-client here until that Interlude pet data is restored — the same class of
//! gap as the Saga htmls. The quest logic is complete and tested against a
//! bound pet regardless.
//!
//! [`QuestCtx::attack_is_summon`]: QuestCtx::attack_is_summon

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const CRONOS: i32 = 30610;
const MIMYU: i32 = 30747;
// Flutes (a hatchling's collar; its enchant level is the hatchling's level)
const DRAGONFLUTE_OF_WIND: i32 = 3500;
const DRAGONFLUTE_OF_STAR: i32 = 3501;
const DRAGONFLUTE_OF_TWILIGHT: i32 = 3502;
const FLUTES: [i32; 3] = [
    DRAGONFLUTE_OF_WIND,
    DRAGONFLUTE_OF_STAR,
    DRAGONFLUTE_OF_TWILIGHT,
];
const FAIRY_LEAF: i32 = 4325;
// Trees of Vision
const TREE_OF_WIND: i32 = 27185;
const TREE_OF_STAR: i32 = 27186;
const TREE_OF_TWILIGHT: i32 = 27187;
const TREE_OF_ABYSS: i32 = 27188;
const SOUL_OF_TREE_GUARDIAN: i32 = 27189;

// Skills the trees and Mimyu cast (Java's `SkillHolder` constants).
/// Mimyu's rebuke to a player presenting the wrong flute.
const CURSE_OF_MIMYU: i32 = 4167;
/// The trees' retaliation poison.
const VICIOUS_POISON: i32 = 4243;
/// The Tree of Abyss's root. **Level 33, not 1** — Java's `SkillHolder`
/// carries the level and it is what sets the root's strength, so a level-1
/// cast here would be a quietly weaker skill.
const DRYAD_ROOT: i32 = 1201;
const DRYAD_ROOT_LEVEL: i32 = 33;
const TREES: [i32; 4] = [TREE_OF_WIND, TREE_OF_STAR, TREE_OF_TWILIGHT, TREE_OF_ABYSS];
// Rewards (the grown strider's whistle)
const DRAGON_BUGLE_OF_WIND: i32 = 4422;
const DRAGON_BUGLE_OF_STAR: i32 = 4423;
const DRAGON_BUGLE_OF_TWILIGHT: i32 = 4424;
// Misc
const MIN_PLAYER_LEVEL: i32 = 45;
const MIN_HATCHLING_LEVEL: i32 = 55;
// The Guardian ambush lingers 5 min before despawning (Java DESPAWN_GUARDIAN).
const GUARDIAN_DESPAWN_MS: u64 = 300_000;

/// Per-tree drink data: `(mod, value, min_hits, taunt)`. `memoState` is a 4-bit
/// field, one bit per tree — `memoState % mod < value` tests whether this tree's
/// bit is still unset, and a successful drink adds `value` to set it. `min_hits`
/// is how long the pet must drink before a leaf can be consumed.
fn tree_data(npc_id: i32) -> Option<(i32, i32, i32, &'static str)> {
    match npc_id {
        TREE_OF_WIND => Some((2, 1, 270, "Hey! You've already drunk the essence of Wind!")),
        TREE_OF_STAR => Some((
            4,
            2,
            400,
            "Hey! You've already drunk the essence of a Star!",
        )),
        TREE_OF_TWILIGHT => Some((8, 4, 150, "Hey! You've already drunk the essence of Dusk!")),
        TREE_OF_ABYSS => Some((
            16,
            8,
            270,
            "Hey! You've already drunk the essence of the Abyss!",
        )),
        _ => None,
    }
}

/// The Dragon Bugle that matches a flute (the strider it grows into).
fn bugle_for_flute(flute_id: i32) -> i32 {
    match flute_id {
        DRAGONFLUTE_OF_WIND => DRAGON_BUGLE_OF_WIND,
        DRAGONFLUTE_OF_STAR => DRAGON_BUGLE_OF_STAR,
        _ => DRAGON_BUGLE_OF_TWILIGHT,
    }
}

pub struct Q00421LittleWingsBigAdventure;

impl Q00421LittleWingsBigAdventure {
    /// `getQuestItemsCount(player, WIND, STAR, TWILIGHT)`.
    fn flute_count(&self, ctx: &QuestCtx) -> i64 {
        FLUTES.iter().map(|&id| ctx.quest_items_count(id)).sum()
    }

    /// `getFlute(player)`'s item id — the single flute the player carries.
    fn flute_item_id(&self, ctx: &QuestCtx) -> Option<i32> {
        FLUTES.into_iter().find(|&id| ctx.quest_items_count(id) > 0)
    }

    /// Whether a *pet* is out and was summoned by the flute this quest is bound
    /// to (`summon.getControlObjectId() == qs.getInt("fluteObjectId")`).
    fn pet_matches_flute(&self, ctx: &QuestCtx) -> bool {
        ctx.pet_control_object_id() == Some(ctx.get_int("fluteObjectId"))
    }
}

impl QuestScript for Q00421LittleWingsBigAdventure {
    fn id(&self) -> i32 {
        421
    }
    fn name(&self) -> &'static str {
        "Q00421_LittleWingsBigAdventure"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00421_LittleWingsBigAdventure"
    }
    fn start_npcs(&self) -> &[i32] {
        &[CRONOS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[CRONOS, MIMYU]
    }
    fn kill_npcs(&self) -> &[i32] {
        &TREES
    }
    fn attack_npcs(&self) -> &[i32] {
        &TREES
    }
    fn quest_items(&self) -> &[i32] {
        &[FAIRY_LEAF]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            CRONOS => {
                if ctx.is_completed() {
                    return Some(ctx.already_completed_html());
                }
                if ctx.is_started() {
                    return Some("30610-07.html".to_string());
                }
                // CREATED: the quest is invisible without a flute in inventory.
                let flute_count = self.flute_count(ctx);
                if flute_count == 0 {
                    return Some(ctx.no_quest_html());
                }
                if ctx.player_level() < MIN_PLAYER_LEVEL {
                    Some("30610-01.htm".to_string())
                } else if flute_count > 1 {
                    Some("30610-02.htm".to_string())
                } else if ctx
                    .item_enchant_level(self.flute_item_id(ctx)?)
                    .unwrap_or(0)
                    < MIN_HATCHLING_LEVEL
                {
                    Some("30610-03.html".to_string())
                } else {
                    Some("30610-04.htm".to_string())
                }
            }
            MIMYU => match ctx.memo_state() {
                100 => {
                    ctx.set_memo_state(200);
                    Some("30747-01.html".to_string())
                }
                200 => {
                    if ctx.pet_control_object_id().is_none() {
                        Some("30747-02.html".to_string())
                    } else if !self.pet_matches_flute(ctx) {
                        Some("30747-03.html".to_string())
                    } else {
                        Some("30747-04.html".to_string())
                    }
                }
                0 => Some("30747-07.html".to_string()),
                1..=14 => {
                    if ctx.quest_items_count(FAIRY_LEAF) > 0 {
                        Some("30747-11.html".to_string())
                    } else {
                        Some(ctx.no_quest_html())
                    }
                }
                15 => {
                    if ctx.quest_items_count(FAIRY_LEAF) > 0 {
                        return Some(ctx.no_quest_html());
                    }
                    if ctx.pet_control_object_id().is_none() {
                        Some("30747-12.html".to_string())
                    } else if self.pet_matches_flute(ctx) {
                        ctx.set_memo_state(16);
                        Some("30747-13.html".to_string())
                    } else {
                        Some("30747-14.html".to_string())
                    }
                }
                16 => {
                    if ctx.quest_items_count(FAIRY_LEAF) > 0 {
                        return Some(ctx.no_quest_html());
                    }
                    // The hatchling must be dismissed before it can be grown.
                    if ctx.has_summon() {
                        return Some("30747-15.html".to_string());
                    }
                    let flute_count = self.flute_count(ctx);
                    if flute_count > 1 {
                        return Some("30747-17.html".to_string());
                    }
                    if flute_count != 1 {
                        return Some(ctx.no_quest_html());
                    }
                    let flute_id = self.flute_item_id(ctx)?;
                    if ctx.item_object_id(flute_id) == Some(ctx.get_int("fluteObjectId")) {
                        ctx.take_items(flute_id, -1);
                        ctx.give_items(bugle_for_flute(flute_id), 1);
                        ctx.exit_quest(true, true);
                        Some("30747-16.html".to_string())
                    } else {
                        // A *different* flute than the one the rite was bound to
                        // — Mimyu curses the impostor.
                        let (npc, player) = (ctx.npc, ctx.player);
                        ctx.npc_cast(npc, player, CURSE_OF_MIMYU, 1);
                        Some("30747-18.html".to_string())
                    }
                }
                _ => Some(ctx.no_quest_html()),
            },
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30610-05.htm" => {
                if !ctx.is_created() {
                    return None;
                }
                if self.flute_count(ctx) != 1 {
                    return Some("30610-06.html".to_string());
                }
                let flute_id = self.flute_item_id(ctx)?;
                if ctx.item_enchant_level(flute_id).unwrap_or(0) < MIN_HATCHLING_LEVEL {
                    return Some("30610-06.html".to_string());
                }
                ctx.start_quest();
                ctx.set_memo_state(100);
                // Bind the rite to *this* flute's object id — a second flute of
                // the same kind is a different hatchling.
                let flute_oid = ctx.item_object_id(flute_id).unwrap_or(0);
                ctx.set_var("fluteObjectId", flute_oid.to_string());
                Some(event.to_string())
            }
            "30747-04.html" => {
                if ctx.pet_control_object_id().is_none() {
                    Some("30747-02.html".to_string())
                } else if !self.pet_matches_flute(ctx) {
                    Some("30747-03.html".to_string())
                } else {
                    Some(event.to_string())
                }
            }
            "30747-05.html" => {
                if ctx.pet_control_object_id().is_none() || !self.pet_matches_flute(ctx) {
                    Some("30747-06.html".to_string())
                } else {
                    ctx.give_items(FAIRY_LEAF, 4);
                    ctx.set_cond(2, true);
                    ctx.set_memo_state(0);
                    Some(event.to_string())
                }
            }
            "30747-07.html" | "30747-08.html" | "30747-09.html" | "30747-10.html" => {
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // Kill a Tree of Vision (rather than merely drinking from it) and its
        // Guardian Ghosts swarm the killer — 20 of them, each despawning after
        // five minutes.
        //
        // Java gates this on `checkIfInRange(ALT_PARTY_RANGE, killer, npc,
        // true)`. That check was omitted here on the grounds that the killer is
        // "in range by construction" — which a ranged or magic kill disproves,
        // and dodging the ambush that way is the point (the `ai/others/
        // FairyTrees` swarm on the same trees has the same 1500-unit gate).
        if tree_data(ctx.npc_id).is_none() {
            return;
        }
        let range = ctx.world.cfg.character.alt_party_range as f64;
        if !crate::geo::distance::within_3d(ctx.world, ctx.npc, ctx.player, range) {
            return;
        }
        for i in 0..20 {
            if let Some(guardian) = ctx.spawn_attacker(SOUL_OF_TREE_GUARDIAN, true) {
                ctx.schedule_despawn(guardian, GUARDIAN_DESPAWN_MS);
            }
            // Java casts inside the loop on the *first* pass — as the first
            // guardian appears, not after all twenty. The dying tree gets one
            // parting shot at its killer.
            if i == 0 {
                let (npc, player) = (ctx.npc, ctx.player);
                ctx.npc_cast(npc, player, VICIOUS_POISON, 1);
            }
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || ctx.cond() != 2 {
            // Below two-thirds HP a tree spits poison at any attacker, quest or
            // no quest. Java evaluates the HP test *before* the roll, so a
            // healthy tree consumes no random value — keep that order.
            if ctx.npc_hp_ratio() < 0.67 && ctx.roll(100) < 30 {
                let (npc, player) = (ctx.npc, ctx.player);
                ctx.npc_cast(npc, player, VICIOUS_POISON, 1);
            }
            return;
        }
        let Some((memo_mod, memo_value, min_hits, taunt)) = tree_data(ctx.npc_id) else {
            return;
        };
        if !ctx.attack_is_summon() {
            // The player struck the tree themselves — no progress; the tree may
            // retaliate with poison. Note this arm has no HP gate: on-quest,
            // a full-health tree still retaliates.
            if ctx.roll(100) < 30 {
                let (npc, player) = (ctx.npc, ctx.player);
                ctx.npc_cast(npc, player, VICIOUS_POISON, 1);
            }
            return;
        }
        // Has this tree already been drunk from? (its bit already set)
        if (ctx.memo_state() % memo_mod) >= memo_value {
            // Already drunk — the tree grumbles rather than pours.
            match ctx.roll(3) {
                0 => ctx.npc_say_text("Why do you bother me again?"),
                1 => ctx.npc_say_text(taunt),
                _ => {
                    ctx.npc_say_text("Leave now, before you incur the wrath of the Guardian Ghost!")
                }
            }
            return;
        }
        // The drink only counts from the bound hatchling.
        if !self.pet_matches_flute(ctx) {
            return;
        }
        let hits = ctx.get_int("hits") + 1;
        ctx.set_var("hits", hits.to_string());
        if hits < min_hits {
            // Only the Tree of Abyss roots, and only on a 2% roll. Dryad Root
            // is cast at level 33, not 1 — Java's SkillHolder says so, and the
            // level sets the root's strength.
            if ctx.npc_id == TREE_OF_ABYSS && ctx.roll(100) < 2 {
                let (npc, player) = (ctx.npc, ctx.player);
                ctx.npc_cast(npc, player, DRYAD_ROOT, DRYAD_ROOT_LEVEL);
            }
            return;
        }
        // Past the threshold: a 2% chance per drink to actually take an essence,
        // consuming one Fairy Leaf and setting this tree's bit.
        if ctx.roll(100) < 2 && ctx.quest_items_count(FAIRY_LEAF) > 0 {
            ctx.npc_say_text("Give me a Fairy Leaf!");
            ctx.take_items(FAIRY_LEAF, 1);
            ctx.set_memo_state(ctx.memo_state() + memo_value);
            ctx.unset("hits");
            ctx.play_sound(quest_sounds::MIDDLE);
            if ctx.memo_state() == 15 {
                ctx.set_cond(3, false);
            }
        }
    }
}
