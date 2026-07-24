//! Test of the Summoner (230) — `quests/Q00230_TestOfTheSummoner`. The proof
//! that turns a Wizard / Elven Wizard / Dark Wizard (level 39+) into a Warlock,
//! Elemental Summoner or Phantom Summoner and earns the Mark of Summoner.
//!
//! Two interlocking halves:
//!  1. **Grocer Lara's lists** (conds 2→3). Galatea's Letter sends the summoner
//!     to Lara, who hands out one of five random hunting lists; each list wants
//!     30 + 30 of two monster tokens (dropped only while the list is held and
//!     the Letter is not). Turning a full pair in yields two Beginner's Arcana.
//!  2. **The arcana duels** (conds 3→4). Each of six Summoner NPCs converts a
//!     Beginner's Arcana into a Crystal of Starting; the summoner then sets their
//!     **servitor** on the matching quest-monster (Pako, Mimi, Unicorn Racer,
//!     Unicorn Phantasm, Shadow Turen, Silhouette Tilfo). The monster only fights
//!     when first struck by a servitor (`onAttack`, `isSummon`) — it swaps the
//!     Starting crystal for In-Progress and strikes back; a servitor kill
//!     (`onKill`, credited to the owner) yields Victory; a player or a *different*
//!     servitor interfering fouls the duel. Each Victory redeemed at its Summoner
//!     becomes one of six Arcana; all six returned to Galatea complete the test.
//!
//! This is the quest the servitor-battle primitives were built for:
//! [`QuestCtx::attack_is_summon`], [`QuestCtx::owner_servitor`],
//! [`QuestCtx::make_npc_attack`] and [`QuestCtx::is_oid_dead`] drive the duel loop.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const GROCER_LARA: i32 = 30063;
const HIGH_SUMMONER_GALATEA: i32 = 30634;
const SUMMONER_ALMORS: i32 = 30635;
const SUMMONER_CAMONIELL: i32 = 30636;
const SUMMONER_BELTHUS: i32 = 30637;
const SUMMONER_BASILLA: i32 = 30638;
const SUMMONER_CELESTIEL: i32 = 30639;
const SUMMONER_BRYNTHEA: i32 = 30640;
// Items — collection tokens
const LETOLIZARDMAN_AMULET: i32 = 3337;
const SAC_OF_REDSPORES: i32 = 3338;
const KARULBUGBEAR_TOTEM: i32 = 3339;
const SHARDS_OF_MANASHEN: i32 = 3340;
const BREKAORC_TOTEM: i32 = 3341;
const CRIMSON_BLOODSTONE: i32 = 3342;
const TALONS_OF_TYRANT: i32 = 3343;
const WINGS_OF_DRONEANT: i32 = 3344;
const TUSK_OF_WINDSUS: i32 = 3345;
const FANGS_OF_WYRM: i32 = 3346;
// Items — Lara's lists
const LARAS_1ST_LIST: i32 = 3347;
const LARAS_2ND_LIST: i32 = 3348;
const LARAS_3RD_LIST: i32 = 3349;
const LARAS_4TH_LIST: i32 = 3350;
const LARAS_5TH_LIST: i32 = 3351;
// Items — letters & arcanas
const GALATEAS_LETTER: i32 = 3352;
const BEGINNERS_ARCANA: i32 = 3353;
const ALMORS_ARCANA: i32 = 3354;
const CAMONIELL_ARCANA: i32 = 3355;
const BELTHUS_ARCANA: i32 = 3356;
const BASILLIA_ARCANA: i32 = 3357;
const CELESTIEL_ARCANA: i32 = 3358;
const BRYNTHEA_ARCANA: i32 = 3359;
// Items — duel crystals (5 states × 6 slots)
const CRYSTAL_OF_STARTING_1ST: i32 = 3360;
const CRYSTAL_OF_INPROGRESS_1ST: i32 = 3361;
const CRYSTAL_OF_FOUL_1ST: i32 = 3362;
const CRYSTAL_OF_DEFEAT_1ST: i32 = 3363;
const CRYSTAL_OF_VICTORY_1ST: i32 = 3364;
const CRYSTAL_OF_STARTING_2ND: i32 = 3365;
const CRYSTAL_OF_INPROGRESS_2ND: i32 = 3366;
const CRYSTAL_OF_FOUL_2ND: i32 = 3367;
const CRYSTAL_OF_DEFEAT_2ND: i32 = 3368;
const CRYSTAL_OF_VICTORY_2ND: i32 = 3369;
const CRYSTAL_OF_STARTING_3RD: i32 = 3370;
const CRYSTAL_OF_INPROGRESS_3RD: i32 = 3371;
const CRYSTAL_OF_FOUL_3RD: i32 = 3372;
const CRYSTAL_OF_DEFEAT_3RD: i32 = 3373;
const CRYSTAL_OF_VICTORY_3RD: i32 = 3374;
const CRYSTAL_OF_STARTING_4TH: i32 = 3375;
const CRYSTAL_OF_INPROGRESS_4TH: i32 = 3376;
const CRYSTAL_OF_FOUL_4TH: i32 = 3377;
const CRYSTAL_OF_DEFEAT_4TH: i32 = 3378;
const CRYSTAL_OF_VICTORY_4TH: i32 = 3379;
const CRYSTAL_OF_STARTING_5TH: i32 = 3380;
const CRYSTAL_OF_INPROGRESS_5TH: i32 = 3381;
const CRYSTAL_OF_FOUL_5TH: i32 = 3382;
const CRYSTAL_OF_DEFEAT_5TH: i32 = 3383;
const CRYSTAL_OF_VICTORY_5TH: i32 = 3384;
const CRYSTAL_OF_STARTING_6TH: i32 = 3385;
const CRYSTAL_OF_INPROGRESS_6TH: i32 = 3386;
const CRYSTAL_OF_FOUL_6TH: i32 = 3387;
const CRYSTAL_OF_DEFEAT_6TH: i32 = 3388;
const CRYSTAL_OF_VICTORY_6TH: i32 = 3389;
// Reward
const MARK_OF_SUMMONER: i32 = 3336;
// Monsters — token farm
const NOBLE_ANT: i32 = 20089;
const NOBLE_ANT_LEADER: i32 = 20090;
const WYRM: i32 = 20176;
const TYRANT: i32 = 20192;
const TYRANT_KINGPIN: i32 = 20193;
const BREKA_ORC: i32 = 20267;
const BREKA_ORC_ARCHER: i32 = 20268;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const BREKA_ORC_WARRIOR: i32 = 20271;
const FETTERED_SOUL: i32 = 20552;
const WINDSUS: i32 = 20553;
const GIANT_FUNGUS: i32 = 20555;
const MANASHEN_GARGOYLE: i32 = 20563;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const KARUL_BUGBEAR: i32 = 20600;
// Quest monsters — the arcana-duel opponents
const PAKO_THE_CAT: i32 = 27102;
const UNICORN_RACER: i32 = 27103;
const SHADOW_TUREN: i32 = 27104;
const MIMI_THE_CAT: i32 = 27105;
const UNICORN_PHANTASM: i32 = 27106;
const SILHOUETTE_TILFO: i32 = 27107;
// NpcStringId battle chatter. TODO(G22): these numeric ids are placeholders
// pending the client NpcString table; the strings themselves are cosmetic
// duel taunts and do not gate any quest state.
const NS_WHHIISSHH: i32 = 23001; // Pako, on engage
const NS_IM_SORRY_LORD: i32 = 23002; // Pako, on defeat
const NS_START_DUEL: i32 = 23003; // Unicorn Racer / Phantasm, on engage
const NS_I_LOSE: i32 = 23004; // Unicorn Racer / Phantasm, on defeat
const NS_SO_SHALL_WE_START: i32 = 23005; // Shadow Turen, on engage
const NS_UGH_I_LOST: i32 = 23006; // Shadow Turen, on defeat
const NS_WHISH_FIGHT: i32 = 23007; // Mimi, on engage
const NS_LOST_SORRY_LORD: i32 = 23008; // Mimi, on defeat
const NS_ILL_WALK_ALL_OVER_YOU: i32 = 23009; // Silhouette Tilfo, on engage
const NS_UGH_CAN_THIS_BE_HAPPENING: i32 = 23010; // Silhouette Tilfo, on defeat
const NS_RULE_VIOLATION: i32 = 23011; // any, on foul
                                      // Misc
const MIN_LEVEL: i32 = 39;
const WIZARD: i32 = 11;
const ELVEN_WIZARD: i32 = 26;
const DARK_WIZARD: i32 = 39;

/// The four crystal ids a single arcana slot cycles through, plus its opponent
/// chatter. Slots are ordered by their crystal-id family (1st..6th), which does
/// **not** match the summoner talk order — the summoner→slot wiring lives in
/// [`Q00230TestOfTheSummoner::on_talk`].
struct Slot {
    starting: i32,
    inprogress: i32,
    foul: i32,
    victory: i32,
    engage_say: i32,
    defeat_say: i32,
}

const SLOT_1ST: Slot = Slot {
    starting: CRYSTAL_OF_STARTING_1ST,
    inprogress: CRYSTAL_OF_INPROGRESS_1ST,
    foul: CRYSTAL_OF_FOUL_1ST,
    victory: CRYSTAL_OF_VICTORY_1ST,
    engage_say: NS_WHHIISSHH,
    defeat_say: NS_IM_SORRY_LORD,
};
const SLOT_2ND: Slot = Slot {
    starting: CRYSTAL_OF_STARTING_2ND,
    inprogress: CRYSTAL_OF_INPROGRESS_2ND,
    foul: CRYSTAL_OF_FOUL_2ND,
    victory: CRYSTAL_OF_VICTORY_2ND,
    engage_say: NS_WHISH_FIGHT,
    defeat_say: NS_LOST_SORRY_LORD,
};
const SLOT_3RD: Slot = Slot {
    starting: CRYSTAL_OF_STARTING_3RD,
    inprogress: CRYSTAL_OF_INPROGRESS_3RD,
    foul: CRYSTAL_OF_FOUL_3RD,
    victory: CRYSTAL_OF_VICTORY_3RD,
    engage_say: NS_START_DUEL,
    defeat_say: NS_I_LOSE,
};
const SLOT_4TH: Slot = Slot {
    starting: CRYSTAL_OF_STARTING_4TH,
    inprogress: CRYSTAL_OF_INPROGRESS_4TH,
    foul: CRYSTAL_OF_FOUL_4TH,
    victory: CRYSTAL_OF_VICTORY_4TH,
    engage_say: NS_START_DUEL,
    defeat_say: NS_I_LOSE,
};
const SLOT_5TH: Slot = Slot {
    starting: CRYSTAL_OF_STARTING_5TH,
    inprogress: CRYSTAL_OF_INPROGRESS_5TH,
    foul: CRYSTAL_OF_FOUL_5TH,
    victory: CRYSTAL_OF_VICTORY_5TH,
    engage_say: NS_SO_SHALL_WE_START,
    defeat_say: NS_UGH_I_LOST,
};
const SLOT_6TH: Slot = Slot {
    starting: CRYSTAL_OF_STARTING_6TH,
    inprogress: CRYSTAL_OF_INPROGRESS_6TH,
    foul: CRYSTAL_OF_FOUL_6TH,
    victory: CRYSTAL_OF_VICTORY_6TH,
    engage_say: NS_ILL_WALK_ALL_OVER_YOU,
    defeat_say: NS_UGH_CAN_THIS_BE_HAPPENING,
};

/// Opponent npc id → its arcana slot.
fn slot_for_opponent(npc_id: i32) -> Option<&'static Slot> {
    Some(match npc_id {
        PAKO_THE_CAT => &SLOT_1ST,
        MIMI_THE_CAT => &SLOT_2ND,
        UNICORN_RACER => &SLOT_3RD,
        UNICORN_PHANTASM => &SLOT_4TH,
        SHADOW_TUREN => &SLOT_5TH,
        SILHOUETTE_TILFO => &SLOT_6TH,
        _ => return None,
    })
}

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// `hasQuestItems(player, a, b, ...)` — every one present.
fn has_all(ctx: &QuestCtx, items: &[i32]) -> bool {
    items.iter().all(|&i| has(ctx, i))
}

/// `hasAtLeastOneQuestItem(player, ...)` — any present.
fn has_any(ctx: &QuestCtx, items: &[i32]) -> bool {
    items.iter().any(|&i| has(ctx, i))
}

/// The six Summoner arcanas, whose full set completes the test.
const ALL_ARCANAS: [i32; 6] = [
    ALMORS_ARCANA,
    CAMONIELL_ARCANA,
    BELTHUS_ARCANA,
    BASILLIA_ARCANA,
    CELESTIEL_ARCANA,
    BRYNTHEA_ARCANA,
];

pub struct Q00230TestOfTheSummoner;

impl Q00230TestOfTheSummoner {
    /// The shared `onAttack` body for every arcana-duel opponent. `scriptValue`
    /// tracks the duel: 0 = unengaged, 1 = a servitor is dueling, 2 = fouled.
    fn duel_attack(&self, ctx: &mut QuestCtx, slot: &Slot) {
        match ctx.npc_script_value() {
            0 => {
                if !ctx.attack_is_summon() {
                    return;
                }
                let Some(servitor) = ctx.owner_servitor() else {
                    return;
                };
                // Any servitor blow starts the duel clock and records the
                // challenger, whether or not the player holds a Starting crystal.
                ctx.set_npc_var_int("ATTACKER", servitor);
                ctx.set_npc_script_value(1);
                ctx.start_quest_timer("DESPAWN", 120_000);
                ctx.start_quest_timer("KILLED_ATTACKER", 5_000);
                if has(ctx, slot.starting) && ctx.is_started() {
                    ctx.npc_say(slot.engage_say);
                    ctx.take_items(slot.starting, -1);
                    ctx.give_items(slot.inprogress, 1);
                    ctx.make_npc_attack(servitor); // addAttackPlayerDesire
                }
            }
            1 => {
                // A foul: the challenger's own player struck, or a *different*
                // servitor butted in.
                if ctx.attack_is_summon()
                    && ctx.owner_servitor() == Some(ctx.npc_var_int("ATTACKER"))
                {
                    return;
                }
                if !has(ctx, slot.starting) && has(ctx, slot.inprogress) && ctx.is_started() {
                    ctx.set_npc_script_value(2);
                    ctx.npc_say(NS_RULE_VIOLATION);
                    ctx.take_items(slot.inprogress, -1);
                    ctx.give_items(slot.foul, 1);
                    ctx.take_items(slot.starting, -1);
                }
                ctx.delete_npc();
            }
            _ => {}
        }
    }

    /// The Summoner-side dialog: convert Beginner's Arcana into a Starting
    /// crystal for `slot`, clearing any prior Foul/Defeat. Java's `-04.html`.
    fn summoner_start_duel(&self, ctx: &mut QuestCtx, slot: &Slot, html: &str) -> Option<String> {
        // TODO(G22): addSkillCastDesire(npc, player, REDUCTION_IN_RECOVERY_TIME
        // (4126,1)) — a cosmetic summon-cooldown buff, no cast-desire helper yet.
        ctx.take_items(BEGINNERS_ARCANA, 1);
        ctx.give_items(slot.starting, 1);
        ctx.take_items(slot.foul, 1);
        ctx.take_items(defeat_crystal(slot), 1);
        Some(html.to_string())
    }
}

impl QuestScript for Q00230TestOfTheSummoner {
    fn id(&self) -> i32 {
        230
    }
    fn name(&self) -> &'static str {
        "Q00230_TestOfTheSummoner"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00230_TestOfTheSummoner"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HIGH_SUMMONER_GALATEA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            HIGH_SUMMONER_GALATEA,
            GROCER_LARA,
            SUMMONER_ALMORS,
            SUMMONER_CAMONIELL,
            SUMMONER_BELTHUS,
            SUMMONER_BASILLA,
            SUMMONER_CELESTIEL,
            SUMMONER_BRYNTHEA,
        ]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[
            PAKO_THE_CAT,
            UNICORN_RACER,
            SHADOW_TUREN,
            MIMI_THE_CAT,
            UNICORN_PHANTASM,
            SILHOUETTE_TILFO,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            NOBLE_ANT,
            NOBLE_ANT_LEADER,
            WYRM,
            TYRANT,
            TYRANT_KINGPIN,
            BREKA_ORC,
            BREKA_ORC_ARCHER,
            BREKA_ORC_SHAMAN,
            BREKA_ORC_OVERLORD,
            BREKA_ORC_WARRIOR,
            FETTERED_SOUL,
            WINDSUS,
            GIANT_FUNGUS,
            MANASHEN_GARGOYLE,
            LETO_LIZARDMAN,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_WARRIOR,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            KARUL_BUGBEAR,
            PAKO_THE_CAT,
            UNICORN_RACER,
            SHADOW_TUREN,
            MIMI_THE_CAT,
            UNICORN_PHANTASM,
            SILHOUETTE_TILFO,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            LETOLIZARDMAN_AMULET,
            SAC_OF_REDSPORES,
            KARULBUGBEAR_TOTEM,
            SHARDS_OF_MANASHEN,
            BREKAORC_TOTEM,
            CRIMSON_BLOODSTONE,
            TALONS_OF_TYRANT,
            WINGS_OF_DRONEANT,
            TUSK_OF_WINDSUS,
            FANGS_OF_WYRM,
            LARAS_1ST_LIST,
            LARAS_2ND_LIST,
            LARAS_3RD_LIST,
            LARAS_4TH_LIST,
            LARAS_5TH_LIST,
            GALATEAS_LETTER,
            BEGINNERS_ARCANA,
            ALMORS_ARCANA,
            CAMONIELL_ARCANA,
            BELTHUS_ARCANA,
            BASILLIA_ARCANA,
            CELESTIEL_ARCANA,
            BRYNTHEA_ARCANA,
            CRYSTAL_OF_STARTING_1ST,
            CRYSTAL_OF_INPROGRESS_1ST,
            CRYSTAL_OF_FOUL_1ST,
            CRYSTAL_OF_DEFEAT_1ST,
            CRYSTAL_OF_VICTORY_1ST,
            CRYSTAL_OF_STARTING_2ND,
            CRYSTAL_OF_INPROGRESS_2ND,
            CRYSTAL_OF_FOUL_2ND,
            CRYSTAL_OF_DEFEAT_2ND,
            CRYSTAL_OF_VICTORY_2ND,
            CRYSTAL_OF_STARTING_3RD,
            CRYSTAL_OF_INPROGRESS_3RD,
            CRYSTAL_OF_FOUL_3RD,
            CRYSTAL_OF_DEFEAT_3RD,
            CRYSTAL_OF_VICTORY_3RD,
            CRYSTAL_OF_STARTING_4TH,
            CRYSTAL_OF_INPROGRESS_4TH,
            CRYSTAL_OF_FOUL_4TH,
            CRYSTAL_OF_DEFEAT_4TH,
            CRYSTAL_OF_VICTORY_4TH,
            CRYSTAL_OF_STARTING_5TH,
            CRYSTAL_OF_INPROGRESS_5TH,
            CRYSTAL_OF_FOUL_5TH,
            CRYSTAL_OF_DEFEAT_5TH,
            CRYSTAL_OF_VICTORY_5TH,
            CRYSTAL_OF_STARTING_6TH,
            CRYSTAL_OF_INPROGRESS_6TH,
            CRYSTAL_OF_FOUL_6TH,
            CRYSTAL_OF_DEFEAT_6TH,
            CRYSTAL_OF_VICTORY_6TH,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // NPC-only duel timers fire with no player attached.
        match event {
            "DESPAWN" => {
                ctx.delete_npc();
                return None;
            }
            "KILLED_ATTACKER" => {
                if ctx.is_oid_dead(ctx.npc_var_int("ATTACKER")) {
                    ctx.delete_npc();
                } else {
                    ctx.start_quest_timer("KILLED_ATTACKER", 5_000);
                }
                return None;
            }
            _ => {}
        }

        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    ctx.play_sound(quest_sounds::MIDDLE);
                    ctx.give_items(GALATEAS_LETTER, 1);
                }
                None
            }
            "30634-04.htm" | "30634-05.htm" | "30634-06.htm" | "30634-07.htm" | "30634-11.html"
            | "30634-11a.html" | "30634-11b.html" | "30634-11c.html" | "30634-11d.html" => {
                Some(event.to_string())
            }
            // Grocer Lara hands out (or re-rolls) a random hunting list.
            "30063-02.html" => {
                give_random_list(ctx);
                ctx.set_cond(2, true);
                ctx.take_items(GALATEAS_LETTER, 1);
                Some(event.to_string())
            }
            "30063-04.html" => {
                give_random_list(ctx);
                Some(event.to_string())
            }
            // Summoner "show me the duel offer" gate: needs a Beginner's Arcana.
            "30635-03.html" => Some(gated_by_arcana(ctx, event, "30635-02.html")),
            "30636-03.html" => Some(gated_by_arcana(ctx, event, "30636-02.html")),
            "30637-03.html" => Some(gated_by_arcana(ctx, event, "30637-02.html")),
            "30638-03.html" => Some(gated_by_arcana(ctx, event, "30638-02.html")),
            "30639-03.html" => Some(gated_by_arcana(ctx, event, "30639-02.html")),
            "30640-03.html" => Some(gated_by_arcana(ctx, event, "30640-02.html")),
            // Summoner "start the duel": Beginner's Arcana → Crystal of Starting.
            "30635-04.html" => self.summoner_start_duel(ctx, &SLOT_1ST, event),
            "30636-04.html" => self.summoner_start_duel(ctx, &SLOT_3RD, event),
            "30637-04.html" => self.summoner_start_duel(ctx, &SLOT_5TH, event),
            "30638-04.html" => self.summoner_start_duel(ctx, &SLOT_2ND, event),
            "30639-04.html" => self.summoner_start_duel(ctx, &SLOT_4TH, event),
            "30640-04.html" => self.summoner_start_duel(ctx, &SLOT_6TH, event),
            _ => None,
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if let Some(slot) = slot_for_opponent(ctx.npc_id) {
            self.duel_attack(ctx, slot);
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        // TODO(G22): Java also gates every branch on
        // Util.checkIfInRange(ALT_PARTY_RANGE, npc, killer, true); range is not
        // modelled in the quest-kill path yet.
        match ctx.npc_id {
            NOBLE_ANT | NOBLE_ANT_LEADER => {
                farm(ctx, LARAS_5TH_LIST, WINGS_OF_DRONEANT, 2);
            }
            WYRM => farm(ctx, LARAS_5TH_LIST, FANGS_OF_WYRM, 3),
            TYRANT | TYRANT_KINGPIN => farm(ctx, LARAS_4TH_LIST, TALONS_OF_TYRANT, 3),
            BREKA_ORC | BREKA_ORC_ARCHER | BREKA_ORC_WARRIOR => {
                farm(ctx, LARAS_3RD_LIST, BREKAORC_TOTEM, 1);
            }
            BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD => {
                farm(ctx, LARAS_3RD_LIST, BREKAORC_TOTEM, 2);
            }
            FETTERED_SOUL => farm(ctx, LARAS_3RD_LIST, CRIMSON_BLOODSTONE, 6),
            WINDSUS => farm(ctx, LARAS_4TH_LIST, TUSK_OF_WINDSUS, 3),
            GIANT_FUNGUS => farm(ctx, LARAS_1ST_LIST, SAC_OF_REDSPORES, 2),
            MANASHEN_GARGOYLE => farm(ctx, LARAS_2ND_LIST, SHARDS_OF_MANASHEN, 2),
            LETO_LIZARDMAN
            | LETO_LIZARDMAN_ARCHER
            | LETO_LIZARDMAN_SOLDIER
            | LETO_LIZARDMAN_WARRIOR => {
                farm(ctx, LARAS_1ST_LIST, LETOLIZARDMAN_AMULET, 1);
            }
            LETO_LIZARDMAN_SHAMAN | LETO_LIZARDMAN_OVERLORD => {
                farm(ctx, LARAS_1ST_LIST, LETOLIZARDMAN_AMULET, 2);
            }
            KARUL_BUGBEAR => farm(ctx, LARAS_2ND_LIST, KARULBUGBEAR_TOTEM, 2),
            id => {
                if let Some(slot) = slot_for_opponent(id) {
                    // A servitor's kill (credited to its owner) claims Victory.
                    if has(ctx, slot.inprogress) {
                        ctx.npc_say(slot.defeat_say);
                        ctx.take_items(slot.inprogress, 1);
                        ctx.give_items(slot.victory, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                    }
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == HIGH_SUMMONER_GALATEA {
                let class = ctx.player_class_id();
                if class == WIZARD || class == ELVEN_WIZARD || class == DARK_WIZARD {
                    return Some(if ctx.player_level() >= MIN_LEVEL {
                        "30634-03.htm".to_string()
                    } else {
                        "30634-02.html".to_string()
                    });
                }
                return Some("30634-01.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == HIGH_SUMMONER_GALATEA {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        // Started.
        let html = match ctx.npc_id {
            HIGH_SUMMONER_GALATEA => galatea_talk(ctx),
            GROCER_LARA => lara_talk(ctx),
            SUMMONER_ALMORS => summoner_talk(ctx, &SLOT_1ST, ALMORS_ARCANA, "30635"),
            SUMMONER_CAMONIELL => summoner_talk(ctx, &SLOT_3RD, CAMONIELL_ARCANA, "30636"),
            SUMMONER_BELTHUS => summoner_talk(ctx, &SLOT_5TH, BELTHUS_ARCANA, "30637"),
            SUMMONER_BASILLA => summoner_talk(ctx, &SLOT_2ND, BASILLIA_ARCANA, "30638"),
            SUMMONER_CELESTIEL => summoner_talk(ctx, &SLOT_4TH, CELESTIEL_ARCANA, "30639"),
            SUMMONER_BRYNTHEA => summoner_talk(ctx, &SLOT_6TH, BRYNTHEA_ARCANA, "30640"),
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }
}

/// `getRandom(5)` → one of Lara's five lists.
fn give_random_list(ctx: &mut QuestCtx) {
    let list = match ctx.roll(5) {
        0 => LARAS_1ST_LIST,
        1 => LARAS_2ND_LIST,
        2 => LARAS_3RD_LIST,
        3 => LARAS_4TH_LIST,
        _ => LARAS_5TH_LIST,
    };
    ctx.give_items(list, 1);
}

/// The Summoner `-03` offer is shown only if a Beginner's Arcana is in hand;
/// otherwise the `-02` "come back with an arcana" page.
fn gated_by_arcana(ctx: &QuestCtx, with_arcana: &str, without: &str) -> String {
    if has(ctx, BEGINNERS_ARCANA) {
        with_arcana.to_string()
    } else {
        without.to_string()
    }
}

/// A token-farm kill: drop `amount` of `token` (cap 30, 100% base rate) only
/// while the matching list is held and Galatea's Letter is not.
fn farm(ctx: &mut QuestCtx, list: i32, token: i32, amount: i64) {
    if !has(ctx, GALATEAS_LETTER) && has(ctx, list) {
        ctx.give_item_randomly(token, amount, 30, 1.0, true);
    }
}

fn galatea_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, GALATEAS_LETTER) {
        return "30634-09.html".to_string();
    }
    if !has_any(ctx, &ALL_ARCANAS) {
        return if has(ctx, BEGINNERS_ARCANA) {
            "30634-11.html".to_string()
        } else {
            "30634-10.html".to_string()
        };
    }
    if has_all(ctx, &ALL_ARCANAS) {
        ctx.give_adena(300_960, true);
        ctx.give_items(MARK_OF_SUMMONER, 1);
        ctx.add_exp_and_sp(1_664_494, 114_220);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
        return "30634-12.html".to_string();
    }
    // Some arcanas but not all, and no Letter: no further dialog in Java.
    ctx.no_quest_html()
}

/// A single Lara list turn-in: both tokens at 30 → consume tokens + list, grant
/// two Beginner's Arcana, advance to cond 3.
fn lara_turn_in(
    ctx: &mut QuestCtx,
    list: i32,
    token_a: i32,
    token_b: i32,
    ok_html: &str,
    wait_html: &str,
) -> String {
    if ctx.quest_items_count(token_a) >= 30 && ctx.quest_items_count(token_b) >= 30 {
        ctx.take_items(token_a, -1);
        ctx.take_items(token_b, -1);
        ctx.take_items(list, 1);
        ctx.give_items(BEGINNERS_ARCANA, 2);
        ctx.set_cond(3, true);
        ok_html.to_string()
    } else {
        wait_html.to_string()
    }
}

fn lara_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, GALATEAS_LETTER) {
        return "30063-01.html".to_string();
    }
    let lists = [
        LARAS_1ST_LIST,
        LARAS_2ND_LIST,
        LARAS_3RD_LIST,
        LARAS_4TH_LIST,
        LARAS_5TH_LIST,
    ];
    if !has_any(ctx, &lists) {
        return "30063-03.html".to_string();
    }
    if has(ctx, LARAS_1ST_LIST) {
        lara_turn_in(
            ctx,
            LARAS_1ST_LIST,
            LETOLIZARDMAN_AMULET,
            SAC_OF_REDSPORES,
            "30063-06.html",
            "30063-05.html",
        )
    } else if has(ctx, LARAS_2ND_LIST) {
        lara_turn_in(
            ctx,
            LARAS_2ND_LIST,
            KARULBUGBEAR_TOTEM,
            SHARDS_OF_MANASHEN,
            "30063-08.html",
            "30063-07.html",
        )
    } else if has(ctx, LARAS_3RD_LIST) {
        lara_turn_in(
            ctx,
            LARAS_3RD_LIST,
            BREKAORC_TOTEM,
            CRIMSON_BLOODSTONE,
            "30063-10.html",
            "30063-09.html",
        )
    } else if has(ctx, LARAS_4TH_LIST) {
        lara_turn_in(
            ctx,
            LARAS_4TH_LIST,
            TALONS_OF_TYRANT,
            TUSK_OF_WINDSUS,
            "30063-12.html",
            "30063-11.html",
        )
    } else if has(ctx, LARAS_5TH_LIST) {
        lara_turn_in(
            ctx,
            LARAS_5TH_LIST,
            WINGS_OF_DRONEANT,
            FANGS_OF_WYRM,
            "30063-14.html",
            "30063-13.html",
        )
    } else {
        ctx.no_quest_html()
    }
}

/// The Summoner-NPC dialog for one arcana slot. Reports the duel's progress from
/// the crystals in hand, and redeems a Victory crystal for the Summoner's arcana
/// (advancing to cond 4 once the other five are already held). `prefix` is the
/// NPC's html id ("30635".."30640").
fn summoner_talk(ctx: &mut QuestCtx, slot: &Slot, arcana: i32, prefix: &str) -> String {
    let defeat = defeat_crystal(slot);
    let html = |n: &str| format!("{prefix}-{n}.html");
    if has(ctx, arcana) {
        return html("10");
    }
    let all = [
        slot.starting,
        slot.inprogress,
        slot.foul,
        defeat,
        slot.victory,
    ];
    if !has_any(ctx, &all) {
        html("01")
    } else if only_has(ctx, &all, defeat) {
        html("05")
    } else if only_has(ctx, &all, slot.foul) {
        html("06")
    } else if only_has(ctx, &all, slot.victory) {
        // Victory redeemed for this Summoner's arcana.
        ctx.give_items(arcana, 1);
        ctx.take_items(slot.victory, 1);
        let others: Vec<i32> = ALL_ARCANAS
            .iter()
            .copied()
            .filter(|&a| a != arcana)
            .collect();
        if has_all(ctx, &others) {
            ctx.set_cond(4, true);
        }
        html("07")
    } else if only_has(ctx, &all, slot.starting) {
        html("08")
    } else if only_has(ctx, &all, slot.inprogress) {
        html("09")
    } else {
        ctx.no_quest_html()
    }
}

/// The Defeat crystal id for a slot (not carried on [`Slot`] since only the
/// Summoner dialog reads it).
fn defeat_crystal(slot: &Slot) -> i32 {
    match slot.starting {
        CRYSTAL_OF_STARTING_1ST => CRYSTAL_OF_DEFEAT_1ST,
        CRYSTAL_OF_STARTING_2ND => CRYSTAL_OF_DEFEAT_2ND,
        CRYSTAL_OF_STARTING_3RD => CRYSTAL_OF_DEFEAT_3RD,
        CRYSTAL_OF_STARTING_4TH => CRYSTAL_OF_DEFEAT_4TH,
        CRYSTAL_OF_STARTING_5TH => CRYSTAL_OF_DEFEAT_5TH,
        _ => CRYSTAL_OF_DEFEAT_6TH,
    }
}

/// `hasQuestItems(one) && !hasAtLeastOneQuestItem(the rest of `all`)` — exactly
/// `one` of the crystal set is held. Mirrors Java's long `!hasAtLeastOneQuestItem(
/// …others…) && hasQuestItems(one)` guards.
fn only_has(ctx: &QuestCtx, all: &[i32], one: i32) -> bool {
    has(ctx, one) && all.iter().all(|&i| i == one || !has(ctx, i))
}
