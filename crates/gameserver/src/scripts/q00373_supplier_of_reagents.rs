//! Supplier of Reagents (373) — `quests/Q00373_SupplierOfReagents`. Wesley
//! (30166, level 57+) hands over a Mixing Stone + Manual; monsters in the
//! Forsaken Plains / Blazing Swamp drop reagent pouches and raw ingredients, and
//! Urn (31149) runs a three-step alchemy UI — pick an **ingredient**, a
//! **catalyst**, then a **temperature** — that mixes them per a fixed formula
//! table into higher reagents (Draconic Essence, Nightmare Oil, Pure Silver…).
//! The ingredient/catalyst choices are carried in the quest-state vars between
//! the dialog pages; the page ids embed the chosen item id.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const WESLEY: i32 = 30166;
const URN: i32 = 31149;
// Kill NPCs.
const CRENDION: i32 = 20813;
const HALLATE_MAID: i32 = 20822;
const HALLATE_GUARDIAN: i32 = 21061;
const PLATINUM_TRIBE_SHAMAN: i32 = 20828;
const PLATINUM_GUARDIAN_SHAMAN: i32 = 21066;
const LAVA_WYRM: i32 = 21111;
const HAMES_ORC_SHAMAN: i32 = 21115;
// Tools.
const MIXING_STONE: i32 = 5904;
const MIXING_MANUAL: i32 = 6317;
// Drops.
const REAGENT_POUCH_1: i32 = 6007;
const REAGENT_POUCH_2: i32 = 6008;
const REAGENT_POUCH_3: i32 = 6009;
const REAGENT_BOX: i32 = 6010;
const WYRMS_BLOOD: i32 = 6011;
const LAVA_STONE: i32 = 6012;
const MOONSTONE_SHARD: i32 = 6013;
const ROTTEN_BONE: i32 = 6014;
const DEMONS_BLOOD: i32 = 6015;
const INFERNIUM_ORE: i32 = 6016;
const BLOOD_ROOT: i32 = 6017;
const VOLCANIC_ASH: i32 = 6018;
const QUICKSILVER: i32 = 6019;
const SULFUR: i32 = 6020;
const DEMONIC_ESSENCE: i32 = 6031;
const MIDNIGHT_OIL: i32 = 6030;
const DRACOPLASM: i32 = 6021;
const MAGMA_DUST: i32 = 6022;
const MOON_DUST: i32 = 6023;
const NECROPLASM: i32 = 6024;
const DEMONPLASM: i32 = 6025;
const INFERNO_DUST: i32 = 6026;
const FIRE_ESSENCE: i32 = 6028;
const LUNARGENT: i32 = 6029;
const DRACONIC_ESSENCE: i32 = 6027;
const ABYSS_OIL: i32 = 6032;
const HELLFIRE_OIL: i32 = 6033;
const NIGHTMARE_OIL: i32 = 6034;
const PURE_SILVER: i32 = 6320;

const INGREDIENT: &str = "ingredient";
const CATALYST: &str = "catalyst";

/// `FORMULAS`: (ingredient amount, ingredient, catalyst, product).
const FORMULAS: [(i64, i32, i32, i32); 15] = [
    (10, WYRMS_BLOOD, BLOOD_ROOT, DRACOPLASM),
    (10, LAVA_STONE, VOLCANIC_ASH, MAGMA_DUST),
    (10, MOONSTONE_SHARD, VOLCANIC_ASH, MOON_DUST),
    (10, ROTTEN_BONE, BLOOD_ROOT, NECROPLASM),
    (10, DEMONS_BLOOD, BLOOD_ROOT, DEMONPLASM),
    (10, INFERNIUM_ORE, VOLCANIC_ASH, INFERNO_DUST),
    (10, DRACOPLASM, QUICKSILVER, DRACONIC_ESSENCE),
    (10, MAGMA_DUST, SULFUR, FIRE_ESSENCE),
    (10, MOON_DUST, QUICKSILVER, LUNARGENT),
    (10, NECROPLASM, QUICKSILVER, MIDNIGHT_OIL),
    (10, DEMONPLASM, SULFUR, DEMONIC_ESSENCE),
    (10, INFERNO_DUST, SULFUR, ABYSS_OIL),
    (1, FIRE_ESSENCE, DEMONIC_ESSENCE, HELLFIRE_OIL),
    (1, LUNARGENT, MIDNIGHT_OIL, NIGHTMARE_OIL),
    (1, LUNARGENT, QUICKSILVER, PURE_SILVER),
];

/// `TEMPERATURES`: (index, success chance /100, product amount). Hotter mixes
/// yield more but succeed less often.
const TEMPERATURES: [(i32, i32, i64); 3] = [(1, 100, 1), (2, 45, 2), (3, 15, 3)];

/// `DROPLIST` — the reagent drops. `Single` rolls out of 1_000_000; `Pair` rolls
/// out of 1000 (item1 below `t2`, item2 up to `t3`, nothing beyond).
enum Drop {
    Single { item: i32, chance: i32 },
    Pair { item1: i32, item2: i32, t2: i32, t3: i32 },
}

fn drop_for(npc_id: i32) -> Option<Drop> {
    Some(match npc_id {
        PLATINUM_GUARDIAN_SHAMAN => Drop::Single { item: REAGENT_BOX, chance: 442000 },
        HAMES_ORC_SHAMAN => Drop::Single { item: REAGENT_POUCH_3, chance: 470000 },
        PLATINUM_TRIBE_SHAMAN => Drop::Pair { item1: REAGENT_POUCH_2, item2: QUICKSILVER, t2: 680, t3: 1000 },
        HALLATE_MAID => Drop::Pair { item1: REAGENT_POUCH_1, item2: VOLCANIC_ASH, t2: 664, t3: 844 },
        HALLATE_GUARDIAN => Drop::Pair { item1: DEMONS_BLOOD, item2: MOONSTONE_SHARD, t2: 729, t3: 833 },
        CRENDION => Drop::Pair { item1: ROTTEN_BONE, item2: QUICKSILVER, t2: 618, t3: 1000 },
        LAVA_WYRM => Drop::Pair { item1: WYRMS_BLOOD, item2: LAVA_STONE, t2: 505, t3: 750 },
        _ => return None,
    })
}

/// The four-digit item id embedded at `event[9..13]` (`31149-03-XXXX` / `-06-`).
fn embedded_id(event: &str) -> Option<i32> {
    event.get(9..13).and_then(|s| s.parse::<i32>().ok())
}

pub struct Q00373SupplierOfReagents;

impl QuestScript for Q00373SupplierOfReagents {
    fn id(&self) -> i32 {
        373
    }
    fn name(&self) -> &'static str {
        "Q00373_SupplierOfReagents"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00373_SupplierOfReagents"
    }
    fn start_npcs(&self) -> &[i32] {
        &[WESLEY]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[WESLEY, URN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            CRENDION,
            HALLATE_MAID,
            HALLATE_GUARDIAN,
            PLATINUM_TRIBE_SHAMAN,
            PLATINUM_GUARDIAN_SHAMAN,
            LAVA_WYRM,
            HAMES_ORC_SHAMAN,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[MIXING_STONE, MIXING_MANUAL]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // Java initialises `htmltext = event` and echoes it for anything it does
        // not specifically rewrite — including a null quest state.
        if !ctx.has_qs() {
            return Some(event.to_string());
        }
        match event {
            "30166-04.htm" => {
                ctx.start_quest();
                ctx.give_items(MIXING_STONE, 1);
                ctx.give_items(MIXING_MANUAL, 1);
                Some(event.to_string())
            }
            "30166-09.htm" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "31149-02.htm" => {
                if ctx.quest_items_count(MIXING_STONE) == 0 {
                    Some("31149-04.htm".to_string())
                } else {
                    Some(event.to_string())
                }
            }
            // Pick an ingredient: valid only if a formula uses it and enough is held.
            s if s.starts_with("31149-03-") => {
                let Some(regent) = embedded_id(s) else {
                    return Some("31149-04.htm".to_string());
                };
                for &(amount, ingredient, ..) in &FORMULAS {
                    if ingredient != regent {
                        continue;
                    }
                    if ctx.quest_items_count(regent) < amount {
                        break;
                    }
                    ctx.set_var(INGREDIENT, regent.to_string());
                    return Some(event.to_string());
                }
                Some("31149-04.htm".to_string())
            }
            // Pick a catalyst: must be held.
            s if s.starts_with("31149-06-") => {
                let Some(catalyst) = embedded_id(s) else {
                    return Some("31149-04.htm".to_string());
                };
                if ctx.quest_items_count(catalyst) == 0 {
                    return Some("31149-04.htm".to_string());
                }
                ctx.set_var(CATALYST, catalyst.to_string());
                Some(event.to_string())
            }
            // Mix at a temperature (index at event[9..10]).
            s if s.starts_with("31149-12-") => {
                let regent = ctx.get_int(INGREDIENT);
                let catalyst = ctx.get_int(CATALYST);
                let temp_index: i32 = s.get(9..10).and_then(|c| c.parse().ok()).unwrap_or(0);
                for &(amount, ingredient, cat, product) in &FORMULAS {
                    if ingredient != regent || cat != catalyst {
                        continue;
                    }
                    if ctx.quest_items_count(regent) < amount
                        || ctx.quest_items_count(catalyst) == 0
                    {
                        break;
                    }
                    ctx.take_items(regent, amount);
                    ctx.take_items(catalyst, 1);
                    for &(index, chance, count) in &TEMPERATURES {
                        if index != temp_index {
                            continue;
                        }
                        if ctx.roll(100) < chance {
                            ctx.give_items(product, count);
                            return Some(format!("31149-12-{product}.htm"));
                        }
                        return Some("31149-11.htm".to_string());
                    }
                }
                Some("31149-13.htm".to_string())
            }
            _ => Some(event.to_string()),
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(player, -1, 3, npc)`. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let Some(drop) = drop_for(ctx.npc_id) else {
            return;
        };
        match drop {
            Drop::Single { item, chance } => {
                if ctx.roll(1_000_000) < chance {
                    ctx.give_items(item, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            Drop::Pair { item1, item2, t2, t3 } => {
                let r = ctx.roll(1000);
                if r < t3 {
                    ctx.give_items(if r < t2 { item1 } else { item2 }, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() < 57 { "30166-01.htm" } else { "30166-02.htm" }.to_string(),
            );
        }
        if ctx.is_started() {
            return Some(if ctx.npc_id == WESLEY { "30166-05.htm" } else { "31149-01.htm" }.to_string());
        }
        Some(ctx.no_quest_html())
    }
}
