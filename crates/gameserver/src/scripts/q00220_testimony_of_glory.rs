//! Testimony of Glory (220) — `quests/Q00220_TestimonyOfGlory`. The Orc
//! 2nd-class prerequisite (Orc race, `ORC_2ND_GROUP`, level 37+). Prefect Vokian
//! and Chianta send the aspirant to subjugate five rival Orc clans — Breka,
//! Enku, Vuku, Turek and Leunt/Tunath — claiming a Scepter from each chief, then
//! to bind the Revenant of the Tantos chief for the Scepter of Tantos and the
//! Ritual Box, earning the Mark of Glory from Flame Lord Kakai.
//!
//! Item-gated (cond 1..11). The five scepter legs (letters from Kasman/Manakia,
//! gloves that summon each clan's champions) complete in any order → cond 5.
//! Java's `onAttack` on the Ragna/Revenant is cosmetic chat over an unread
//! `scriptValue`, so it is not ported. Two Java copy-paste `==` sound checks
//! (`TYRANT_TALON == 29`, `MANASHEN_SHARD == 19`) are kept faithfully — they
//! only misfire a sound. Radar markers are dropped (client-only).

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const PREFECT_KASMAN: i32 = 30501;
const PREFECT_VOKIAN: i32 = 30514;
const SEER_MANAKIA: i32 = 30515;
const FLAME_LORD_KAKAI: i32 = 30565;
const SEER_TANAPI: i32 = 30571;
const BREKA_CHIEF_VOLTAR: i32 = 30615;
const ENKU_CHIEF_KEPRA: i32 = 30616;
const TUREK_CHIEF_BURAI: i32 = 30617;
const LEUNT_CHIEF_HARAK: i32 = 30618;
const VUKU_CHIEF_DRIKO: i32 = 30619;
const GANDI_CHIEF_CHIANTA: i32 = 30642;
// Items
const VOKIANS_ORDER: i32 = 3204;
const MANASHEN_SHARD: i32 = 3205;
const TYRANT_TALON: i32 = 3206;
const GUARDIAN_BASILISK_FANG: i32 = 3207;
const VOKIANS_ORDER2: i32 = 3208;
const NECKLACE_OF_AUTHORITY: i32 = 3209;
const CHIANTA_1ST_ORDER: i32 = 3210;
const SCEPTER_OF_BREKA: i32 = 3211;
const SCEPTER_OF_ENKU: i32 = 3212;
const SCEPTER_OF_VUKU: i32 = 3213;
const SCEPTER_OF_TUREK: i32 = 3214;
const SCEPTER_OF_TUNATH: i32 = 3215;
const CHIANTA_3RD_ORDER: i32 = 3217;
const TAMLIN_ORC_SKULL: i32 = 3218;
const TIMAK_ORC_HEAD: i32 = 3219;
const SCEPTER_BOX: i32 = 3220;
const PASHIKAS_HEAD: i32 = 3221;
const VULTUS_HEAD: i32 = 3222;
const GLOVE_OF_VOLTAR: i32 = 3223;
const ENKU_OVERLORD_HEAD: i32 = 3224;
const GLOVE_OF_KEPRA: i32 = 3225;
const MAKUM_BUGBEAR_HEAD: i32 = 3226;
const GLOVE_OF_BURAI: i32 = 3227;
const MANAKIA_1ST_LETTER: i32 = 3228;
const MANAKIA_2ND_LETTER: i32 = 3229;
const KASMANS_1ST_LETTER: i32 = 3230;
const KASMANS_2ND_LETTER: i32 = 3231;
const KASMANS_3RD_LETTER: i32 = 3232;
const DRIKOS_CONTRACT: i32 = 3233;
const STAKATO_DRONE_HUSK: i32 = 3234;
const TANAPIS_ORDER: i32 = 3235;
const SCEPTER_OF_TANTOS: i32 = 3236;
const RITUAL_BOX: i32 = 3237;
// Reward
const MARK_OF_GLORY: i32 = 3203;
// Monsters
const TYRANT: i32 = 20192;
const TYRANT_KINGPIN: i32 = 20193;
const MARSH_STAKATO_DRONE: i32 = 20234;
const GUARDIAN_BASILISK: i32 = 20550;
const MANASHEN_GARGOYLE: i32 = 20563;
const TIMAK_ORC: i32 = 20583;
const TIMAK_ORC_ARCHER: i32 = 20584;
const TIMAK_ORC_SOLDIER: i32 = 20585;
const TIMAK_ORC_WARRIOR: i32 = 20586;
const TIMAK_ORC_SHAMAN: i32 = 20587;
const TIMAK_ORC_OVERLORD: i32 = 20588;
const TAMLIN_ORC: i32 = 20601;
const TAMLIN_ORC_ARCHER: i32 = 20602;
const RAGNA_ORC_OVERLORD: i32 = 20778;
const RAGNA_ORC_SEER: i32 = 20779;
const PASHIKA_SON_OF_VOLTAR: i32 = 27080;
const VULTUS_SON_OF_VOLTAR: i32 = 27081;
const ENKU_ORC_OVERLORD: i32 = 27082;
const MAKUM_BUGBEAR_THUG: i32 = 27083;
const REVENANT_OF_TANTOS_CHIEF: i32 = 27086;
// Misc
const MIN_LEVEL: i32 = 37;
const RACE_ORC: i32 = 3;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

fn all_five_scepters(ctx: &QuestCtx) -> bool {
    has(ctx, SCEPTER_OF_BREKA)
        && has(ctx, SCEPTER_OF_ENKU)
        && has(ctx, SCEPTER_OF_VUKU)
        && has(ctx, SCEPTER_OF_TUREK)
        && has(ctx, SCEPTER_OF_TUNATH)
}

/// One of Vokian's three subjugation reagents; cond 2 once all three reach 10.
fn vokian_reagent(ctx: &mut QuestCtx, own: i32, a: i32, b: i32) {
    if has(ctx, VOKIANS_ORDER) && ctx.quest_items_count(own) < 10 {
        if ctx.quest_items_count(own) == 9 {
            ctx.give_items(own, 1);
            ctx.play_sound(quest_sounds::MIDDLE);
            if ctx.quest_items_count(a) >= 10 && ctx.quest_items_count(b) >= 10 {
                ctx.set_cond(2, false);
            }
        } else {
            ctx.give_items(own, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

pub struct Q00220TestimonyOfGlory;

impl QuestScript for Q00220TestimonyOfGlory {
    fn id(&self) -> i32 {
        220
    }
    fn name(&self) -> &'static str {
        "Q00220_TestimonyOfGlory"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00220_TestimonyOfGlory"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PREFECT_VOKIAN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            PREFECT_VOKIAN,
            PREFECT_KASMAN,
            SEER_MANAKIA,
            FLAME_LORD_KAKAI,
            SEER_TANAPI,
            BREKA_CHIEF_VOLTAR,
            ENKU_CHIEF_KEPRA,
            TUREK_CHIEF_BURAI,
            LEUNT_CHIEF_HARAK,
            VUKU_CHIEF_DRIKO,
            GANDI_CHIEF_CHIANTA,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            TYRANT,
            TYRANT_KINGPIN,
            MARSH_STAKATO_DRONE,
            GUARDIAN_BASILISK,
            MANASHEN_GARGOYLE,
            TIMAK_ORC,
            TIMAK_ORC_ARCHER,
            TIMAK_ORC_SOLDIER,
            TIMAK_ORC_WARRIOR,
            TIMAK_ORC_SHAMAN,
            TIMAK_ORC_OVERLORD,
            TAMLIN_ORC,
            TAMLIN_ORC_ARCHER,
            RAGNA_ORC_OVERLORD,
            RAGNA_ORC_SEER,
            PASHIKA_SON_OF_VOLTAR,
            VULTUS_SON_OF_VOLTAR,
            ENKU_ORC_OVERLORD,
            MAKUM_BUGBEAR_THUG,
            REVENANT_OF_TANTOS_CHIEF,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            VOKIANS_ORDER,
            MANASHEN_SHARD,
            TYRANT_TALON,
            GUARDIAN_BASILISK_FANG,
            VOKIANS_ORDER2,
            NECKLACE_OF_AUTHORITY,
            CHIANTA_1ST_ORDER,
            SCEPTER_OF_BREKA,
            SCEPTER_OF_ENKU,
            SCEPTER_OF_VUKU,
            SCEPTER_OF_TUREK,
            SCEPTER_OF_TUNATH,
            CHIANTA_3RD_ORDER,
            TAMLIN_ORC_SKULL,
            TIMAK_ORC_HEAD,
            SCEPTER_BOX,
            PASHIKAS_HEAD,
            VULTUS_HEAD,
            GLOVE_OF_VOLTAR,
            ENKU_OVERLORD_HEAD,
            GLOVE_OF_KEPRA,
            MAKUM_BUGBEAR_HEAD,
            GLOVE_OF_BURAI,
            MANAKIA_1ST_LETTER,
            MANAKIA_2ND_LETTER,
            KASMANS_1ST_LETTER,
            KASMANS_2ND_LETTER,
            KASMANS_3RD_LETTER,
            DRIKOS_CONTRACT,
            STAKATO_DRONE_HUSK,
            TANAPIS_ORDER,
            SCEPTER_OF_TANTOS,
            RITUAL_BOX,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    ctx.play_sound(quest_sounds::MIDDLE);
                    ctx.give_items(VOKIANS_ORDER, 1);
                }
                None
            }
            "30514-04.htm" | "30514-07.html" | "30571-02.html" | "30615-03.html"
            | "30616-03.html" | "30642-02.html" | "30642-06.html" | "30642-08.html" => {
                Some(event.to_string())
            }
            // Kasman's three letters (radar markers dropped).
            "30501-02.html" => kasman_letter(
                ctx,
                SCEPTER_OF_VUKU,
                KASMANS_1ST_LETTER,
                DRIKOS_CONTRACT,
                "30501-03.html",
                "30501-04.html",
                event,
            ),
            "30501-05.html" => kasman_letter(
                ctx,
                SCEPTER_OF_TUREK,
                KASMANS_2ND_LETTER,
                0,
                "30501-06.html",
                "30501-07.html",
                event,
            ),
            "30501-08.html" => kasman_letter(
                ctx,
                SCEPTER_OF_TUNATH,
                KASMANS_3RD_LETTER,
                0,
                "30501-09.html",
                "30501-10.html",
                event,
            ),
            "30515-04.html" => {
                if !has(ctx, SCEPTER_OF_BREKA) && has(ctx, MANAKIA_1ST_LETTER) {
                    Some("30515-04.html".to_string())
                } else if has(ctx, SCEPTER_OF_BREKA) {
                    Some("30515-02.html".to_string())
                } else if !has(ctx, MANAKIA_1ST_LETTER) {
                    ctx.give_items(MANAKIA_1ST_LETTER, 1);
                    Some("30515-03.html".to_string())
                } else {
                    None
                }
            }
            "30515-05.html" => {
                if has(ctx, SCEPTER_OF_ENKU) {
                    Some("30515-05.html".to_string())
                } else if !has(ctx, MANAKIA_2ND_LETTER) {
                    ctx.give_items(MANAKIA_2ND_LETTER, 1);
                    Some("30515-06.html".to_string())
                } else {
                    Some("30515-07.html".to_string())
                }
            }
            "30571-03.html" => ctx
                .swap_quest_item(SCEPTER_BOX, TANAPIS_ORDER, 9)
                .then(|| event.to_string()),
            "30615-04.html" => {
                chief_trade(ctx, MANAKIA_1ST_LETTER, GLOVE_OF_VOLTAR, event, |ctx| {
                    ctx.spawn_attacker(PASHIKA_SON_OF_VOLTAR, true);
                    ctx.spawn_attacker(VULTUS_SON_OF_VOLTAR, true);
                })
            }
            "30616-04.html" => chief_trade(ctx, MANAKIA_2ND_LETTER, GLOVE_OF_KEPRA, event, |ctx| {
                for _ in 0..4 {
                    ctx.spawn_attacker(ENKU_ORC_OVERLORD, true);
                }
            }),
            "30617-03.html" => chief_trade(ctx, KASMANS_2ND_LETTER, GLOVE_OF_BURAI, event, |ctx| {
                ctx.spawn_attacker(MAKUM_BUGBEAR_THUG, true);
                ctx.spawn_attacker(MAKUM_BUGBEAR_THUG, true);
            }),
            "30618-03.html" => chief_trade(
                ctx,
                KASMANS_3RD_LETTER,
                SCEPTER_OF_TUNATH,
                event,
                scepter_claimed,
            ),
            "30619-03.html" => chief_trade(ctx, KASMANS_1ST_LETTER, DRIKOS_CONTRACT, event, |_| {}),
            "30642-03.html" => ctx
                .swap_quest_item(VOKIANS_ORDER2, CHIANTA_1ST_ORDER, 4)
                .then(|| event.to_string()),
            "30642-07.html" => {
                if has(ctx, CHIANTA_1ST_ORDER) && all_five_scepters(ctx) {
                    ctx.take_items(CHIANTA_1ST_ORDER, 1);
                    ctx.take_items(SCEPTER_OF_BREKA, 1);
                    ctx.take_items(SCEPTER_OF_ENKU, 1);
                    ctx.take_items(SCEPTER_OF_VUKU, 1);
                    ctx.take_items(SCEPTER_OF_TUREK, 1);
                    ctx.take_items(SCEPTER_OF_TUNATH, 1);
                    ctx.take_items(MANAKIA_1ST_LETTER, 1);
                    ctx.take_items(MANAKIA_2ND_LETTER, 1);
                    ctx.take_items(KASMANS_1ST_LETTER, 1);
                    ctx.give_items(CHIANTA_3RD_ORDER, 1);
                    ctx.set_cond(6, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            TYRANT | TYRANT_KINGPIN => {
                vokian_reagent(ctx, TYRANT_TALON, MANASHEN_SHARD, GUARDIAN_BASILISK_FANG)
            }
            GUARDIAN_BASILISK => {
                vokian_reagent(ctx, GUARDIAN_BASILISK_FANG, MANASHEN_SHARD, TYRANT_TALON)
            }
            MANASHEN_GARGOYLE => {
                vokian_reagent(ctx, MANASHEN_SHARD, TYRANT_TALON, GUARDIAN_BASILISK_FANG)
            }
            MARSH_STAKATO_DRONE => {
                if !has(ctx, SCEPTER_OF_VUKU)
                    && has(ctx, NECKLACE_OF_AUTHORITY)
                    && has(ctx, CHIANTA_1ST_ORDER)
                    && has(ctx, DRIKOS_CONTRACT)
                    && ctx.quest_items_count(STAKATO_DRONE_HUSK) < 30
                {
                    ctx.give_items(STAKATO_DRONE_HUSK, 1);
                    // NB: Java checks TYRANT_TALON == 29 here (copy-paste); kept.
                    ctx.play_sound(if ctx.quest_items_count(TYRANT_TALON) == 29 {
                        quest_sounds::MIDDLE
                    } else {
                        quest_sounds::ITEMGET
                    });
                }
            }
            TIMAK_ORC | TIMAK_ORC_ARCHER | TIMAK_ORC_SOLDIER | TIMAK_ORC_WARRIOR
            | TIMAK_ORC_SHAMAN | TIMAK_ORC_OVERLORD => {
                if has(ctx, NECKLACE_OF_AUTHORITY)
                    && has(ctx, CHIANTA_3RD_ORDER)
                    && ctx.quest_items_count(TIMAK_ORC_HEAD) < 20
                {
                    ctx.give_items(TIMAK_ORC_HEAD, 1);
                    // NB: Java checks MANASHEN_SHARD == 19 here (copy-paste); kept.
                    if ctx.quest_items_count(MANASHEN_SHARD) == 19 {
                        ctx.play_sound(quest_sounds::MIDDLE);
                        if ctx.quest_items_count(TAMLIN_ORC_SKULL) >= 20 {
                            ctx.set_cond(7, false);
                        }
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            TAMLIN_ORC | TAMLIN_ORC_ARCHER => {
                if has(ctx, NECKLACE_OF_AUTHORITY)
                    && has(ctx, CHIANTA_3RD_ORDER)
                    && ctx.quest_items_count(TAMLIN_ORC_SKULL) < 20
                {
                    if ctx.quest_items_count(TAMLIN_ORC_SKULL) == 19 {
                        ctx.give_items(TAMLIN_ORC_SKULL, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                        if ctx.quest_items_count(TIMAK_ORC_HEAD) >= 20 {
                            ctx.set_cond(7, false);
                        }
                    } else {
                        ctx.give_items(TAMLIN_ORC_SKULL, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            RAGNA_ORC_OVERLORD | RAGNA_ORC_SEER => {
                if has(ctx, TANAPIS_ORDER) && !has(ctx, SCEPTER_OF_TANTOS) {
                    ctx.spawn_attacker(REVENANT_OF_TANTOS_CHIEF, true);
                }
            }
            PASHIKA_SON_OF_VOLTAR => {
                champion_head(ctx, PASHIKAS_HEAD, VULTUS_HEAD, GLOVE_OF_VOLTAR)
            }
            VULTUS_SON_OF_VOLTAR => champion_head(ctx, VULTUS_HEAD, PASHIKAS_HEAD, GLOVE_OF_VOLTAR),
            ENKU_ORC_OVERLORD => {
                if has(ctx, NECKLACE_OF_AUTHORITY)
                    && has(ctx, CHIANTA_1ST_ORDER)
                    && has(ctx, GLOVE_OF_KEPRA)
                    && ctx.quest_items_count(ENKU_OVERLORD_HEAD) < 4
                {
                    if ctx.quest_items_count(ENKU_OVERLORD_HEAD) == 3 {
                        ctx.give_items(ENKU_OVERLORD_HEAD, 1);
                        ctx.take_items(GLOVE_OF_KEPRA, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                    } else {
                        ctx.give_items(ENKU_OVERLORD_HEAD, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            MAKUM_BUGBEAR_THUG => {
                if has(ctx, NECKLACE_OF_AUTHORITY)
                    && has(ctx, CHIANTA_1ST_ORDER)
                    && has(ctx, GLOVE_OF_BURAI)
                    && ctx.quest_items_count(MAKUM_BUGBEAR_HEAD) < 2
                {
                    if ctx.quest_items_count(MAKUM_BUGBEAR_HEAD) == 1 {
                        ctx.give_items(MAKUM_BUGBEAR_HEAD, 1);
                        ctx.take_items(GLOVE_OF_BURAI, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                    } else {
                        ctx.give_items(MAKUM_BUGBEAR_HEAD, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            REVENANT_OF_TANTOS_CHIEF => {
                if has(ctx, TANAPIS_ORDER) && !has(ctx, SCEPTER_OF_TANTOS) {
                    ctx.give_items(SCEPTER_OF_TANTOS, 1);
                    ctx.set_cond(10, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == PREFECT_VOKIAN {
                if ctx.player_race() != RACE_ORC {
                    return Some("30514-01.html".to_string());
                } else if ctx.player_level() < MIN_LEVEL {
                    return Some("30514-02.html".to_string());
                } else if ctx.is_in_category("ORC_2ND_GROUP") {
                    return Some("30514-03.htm".to_string());
                }
                return Some("30514-01a.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == PREFECT_VOKIAN {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            PREFECT_VOKIAN => Some(vokian_talk(ctx)),
            PREFECT_KASMAN => Some(
                if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_1ST_ORDER) {
                    "30501-01.html".to_string()
                } else {
                    ctx.no_quest_html()
                },
            ),
            SEER_MANAKIA => Some(
                if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_1ST_ORDER) {
                    "30515-01.html".to_string()
                } else {
                    ctx.no_quest_html()
                },
            ),
            FLAME_LORD_KAKAI => Some(kakai_talk(ctx)),
            SEER_TANAPI => Some(tanapi_talk(ctx)),
            BREKA_CHIEF_VOLTAR => Some(voltar_talk(ctx)),
            ENKU_CHIEF_KEPRA => Some(kepra_talk(ctx)),
            TUREK_CHIEF_BURAI => Some(burai_talk(ctx)),
            LEUNT_CHIEF_HARAK => Some(harak_talk(ctx)),
            VUKU_CHIEF_DRIKO => Some(driko_talk(ctx)),
            GANDI_CHIEF_CHIANTA => Some(chianta_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

/// A Kasman letter branch: already have the scepter → info page; else grant the
/// letter (radar marker in Java, dropped here); else re-show the reminder.
fn kasman_letter(
    ctx: &mut QuestCtx,
    scepter: i32,
    letter: i32,
    also: i32,
    grant_html: &str,
    remind_html: &str,
    have_html: &str,
) -> Option<String> {
    if has(ctx, scepter) {
        Some(have_html.to_string())
    } else if !has(ctx, letter) && (also == 0 || !has(ctx, also)) {
        ctx.give_items(letter, 1);
        Some(grant_html.to_string())
    } else if has(ctx, letter) || (also != 0 && has(ctx, also)) {
        Some(remind_html.to_string())
    } else {
        None
    }
}

/// A clan chief's trade: the letter (or Kasman's contract) buys the promised
/// glove or scepter, then `after` runs the branch's own follow-up — the
/// champions the glove summons, or the cond the last scepter completes.
fn chief_trade(
    ctx: &mut QuestCtx,
    letter: i32,
    reward: i32,
    event: &str,
    after: impl FnOnce(&mut QuestCtx),
) -> Option<String> {
    if !has(ctx, letter) {
        return None;
    }
    ctx.give_items(reward, 1);
    ctx.take_items(letter, 1);
    after(ctx);
    Some(event.to_string())
}

/// Called once a scepter has just been handed over: the five legs complete in
/// any order, so each one re-checks the whole set for cond 5.
fn scepter_claimed(ctx: &mut QuestCtx) {
    if all_five_scepters(ctx) {
        ctx.set_cond(5, true);
    }
}

/// Pashika/Vultus (Breka champions): each drops its head; when both are held,
/// the second consumes Voltar's Glove.
fn champion_head(ctx: &mut QuestCtx, own: i32, other: i32, glove: i32) {
    if has(ctx, NECKLACE_OF_AUTHORITY)
        && has(ctx, CHIANTA_1ST_ORDER)
        && has(ctx, glove)
        && !has(ctx, own)
    {
        if has(ctx, other) {
            ctx.give_items(own, 1);
            ctx.take_items(glove, 1);
            ctx.play_sound(quest_sounds::MIDDLE);
        } else {
            ctx.give_items(own, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

fn vokian_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, VOKIANS_ORDER) {
        if ctx.quest_items_count(MANASHEN_SHARD) >= 10
            && ctx.quest_items_count(TYRANT_TALON) >= 10
            && ctx.quest_items_count(GUARDIAN_BASILISK_FANG) >= 10
        {
            ctx.take_items(VOKIANS_ORDER, 1);
            ctx.take_items(MANASHEN_SHARD, -1);
            ctx.take_items(TYRANT_TALON, -1);
            ctx.take_items(GUARDIAN_BASILISK_FANG, -1);
            ctx.give_items(VOKIANS_ORDER2, 1);
            ctx.give_items(NECKLACE_OF_AUTHORITY, 1);
            ctx.set_cond(3, true);
            "30514-08.html".to_string()
        } else {
            "30514-06.html".to_string()
        }
    } else if has(ctx, VOKIANS_ORDER2) && has(ctx, NECKLACE_OF_AUTHORITY) {
        "30514-09.html".to_string()
    } else if !has(ctx, NECKLACE_OF_AUTHORITY)
        && (has(ctx, VOKIANS_ORDER2) || has(ctx, SCEPTER_BOX))
    {
        "30514-10.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn kakai_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, RITUAL_BOX) && (has(ctx, SCEPTER_BOX) || has(ctx, TANAPIS_ORDER)) {
        "30565-01.html".to_string()
    } else if has(ctx, RITUAL_BOX) {
        ctx.give_adena(262720, true);
        ctx.give_items(MARK_OF_GLORY, 1);
        ctx.add_exp_and_sp(1448226, 96648);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
        "30565-02.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn tanapi_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, SCEPTER_BOX) {
        "30571-01.html".to_string()
    } else if has(ctx, TANAPIS_ORDER) {
        if !has(ctx, SCEPTER_OF_TANTOS) {
            "30571-04.html".to_string()
        } else {
            ctx.take_items(TANAPIS_ORDER, 1);
            ctx.take_items(SCEPTER_OF_TANTOS, 1);
            ctx.give_items(RITUAL_BOX, 1);
            ctx.set_cond(11, true);
            "30571-05.html".to_string()
        }
    } else if has(ctx, RITUAL_BOX) {
        "30571-06.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn voltar_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_1ST_ORDER) {
        if !has(ctx, SCEPTER_OF_BREKA)
            && !has(ctx, MANAKIA_1ST_LETTER)
            && !has(ctx, GLOVE_OF_VOLTAR)
            && !has(ctx, PASHIKAS_HEAD)
            && !has(ctx, VULTUS_HEAD)
        {
            "30615-01.html".to_string()
        } else if has(ctx, MANAKIA_1ST_LETTER) {
            "30615-02.html".to_string()
        } else if !has(ctx, SCEPTER_OF_BREKA)
            && has(ctx, GLOVE_OF_VOLTAR)
            && (ctx.quest_items_count(PASHIKAS_HEAD) + ctx.quest_items_count(VULTUS_HEAD)) < 2
        {
            ctx.spawn_attacker(PASHIKA_SON_OF_VOLTAR, true);
            ctx.spawn_attacker(VULTUS_SON_OF_VOLTAR, true);
            "30615-05.html".to_string()
        } else if has(ctx, PASHIKAS_HEAD) && has(ctx, VULTUS_HEAD) {
            ctx.give_items(SCEPTER_OF_BREKA, 1);
            ctx.take_items(PASHIKAS_HEAD, 1);
            ctx.take_items(VULTUS_HEAD, 1);
            scepter_claimed(ctx);
            "30615-06.html".to_string()
        } else if has(ctx, SCEPTER_OF_BREKA) {
            "30615-07.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn kepra_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_1ST_ORDER) {
        if !has(ctx, SCEPTER_OF_ENKU)
            && !has(ctx, MANAKIA_2ND_LETTER)
            && !has(ctx, GLOVE_OF_KEPRA)
            && ctx.quest_items_count(ENKU_OVERLORD_HEAD) < 4
        {
            "30616-01.html".to_string()
        } else if has(ctx, MANAKIA_2ND_LETTER) {
            "30616-02.html".to_string()
        } else if has(ctx, GLOVE_OF_KEPRA) && ctx.quest_items_count(ENKU_OVERLORD_HEAD) < 4 {
            ctx.spawn_attacker(ENKU_ORC_OVERLORD, true);
            "30616-05.html".to_string()
        } else if ctx.quest_items_count(ENKU_OVERLORD_HEAD) >= 4 {
            ctx.give_items(SCEPTER_OF_ENKU, 1);
            ctx.take_items(ENKU_OVERLORD_HEAD, -1);
            scepter_claimed(ctx);
            "30616-06.html".to_string()
        } else if has(ctx, SCEPTER_OF_ENKU) {
            "30616-07.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn burai_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_1ST_ORDER) {
        if !has(ctx, SCEPTER_OF_TUREK)
            && !has(ctx, KASMANS_2ND_LETTER)
            && !has(ctx, GLOVE_OF_BURAI)
            && !has(ctx, MAKUM_BUGBEAR_HEAD)
        {
            "30617-01.html".to_string()
        } else if has(ctx, KASMANS_2ND_LETTER) {
            "30617-02.html".to_string()
        } else if has(ctx, GLOVE_OF_BURAI) {
            ctx.spawn_attacker(MAKUM_BUGBEAR_THUG, true);
            ctx.spawn_attacker(MAKUM_BUGBEAR_THUG, true);
            "30617-04.html".to_string()
        } else if ctx.quest_items_count(MAKUM_BUGBEAR_HEAD) >= 2 {
            ctx.give_items(SCEPTER_OF_TUREK, 1);
            ctx.take_items(MAKUM_BUGBEAR_HEAD, -1);
            scepter_claimed(ctx);
            "30617-05.html".to_string()
        } else if has(ctx, SCEPTER_OF_TUREK) {
            "30617-06.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn harak_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_1ST_ORDER) {
        if !has(ctx, SCEPTER_OF_TUNATH) && !has(ctx, KASMANS_3RD_LETTER) {
            "30618-01.html".to_string()
        } else if !has(ctx, SCEPTER_OF_TUNATH) && has(ctx, KASMANS_3RD_LETTER) {
            "30618-02.html".to_string()
        } else if has(ctx, SCEPTER_OF_TUNATH) {
            "30618-04.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn driko_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_1ST_ORDER) {
        if !has(ctx, SCEPTER_OF_VUKU) && !has(ctx, KASMANS_1ST_LETTER) && !has(ctx, DRIKOS_CONTRACT)
        {
            "30619-01.html".to_string()
        } else if !has(ctx, SCEPTER_OF_VUKU) && has(ctx, KASMANS_1ST_LETTER) {
            "30619-02.html".to_string()
        } else if !has(ctx, SCEPTER_OF_VUKU) && has(ctx, DRIKOS_CONTRACT) {
            if ctx.quest_items_count(STAKATO_DRONE_HUSK) < 30 {
                "30619-04.html".to_string()
            } else {
                ctx.give_items(SCEPTER_OF_VUKU, 1);
                ctx.take_items(DRIKOS_CONTRACT, 1);
                ctx.take_items(STAKATO_DRONE_HUSK, -1);
                scepter_claimed(ctx);
                "30619-05.html".to_string()
            }
        } else if has(ctx, SCEPTER_OF_VUKU) {
            "30619-06.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn chianta_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, VOKIANS_ORDER2) {
        "30642-01.html".to_string()
    } else if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_1ST_ORDER) {
        if all_five_scepters(ctx) {
            "30642-05.html".to_string()
        } else {
            "30642-04.html".to_string()
        }
    } else if has(ctx, NECKLACE_OF_AUTHORITY) && has(ctx, CHIANTA_3RD_ORDER) {
        if ctx.quest_items_count(TAMLIN_ORC_SKULL) >= 20
            && ctx.quest_items_count(TIMAK_ORC_HEAD) >= 20
        {
            ctx.take_items(NECKLACE_OF_AUTHORITY, 1);
            ctx.take_items(CHIANTA_3RD_ORDER, 1);
            ctx.take_items(TAMLIN_ORC_SKULL, -1);
            ctx.take_items(TIMAK_ORC_HEAD, -1);
            ctx.give_items(SCEPTER_BOX, 1);
            ctx.set_cond(8, true);
            "30642-11.html".to_string()
        } else {
            "30642-10.html".to_string()
        }
    } else if has(ctx, SCEPTER_BOX) {
        "30642-12.html".to_string()
    } else if has(ctx, TANAPIS_ORDER) || has(ctx, RITUAL_BOX) {
        "30642-13.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
