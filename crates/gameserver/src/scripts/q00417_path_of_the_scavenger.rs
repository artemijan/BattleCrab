//! Path Of The Scavenger (417) — port of
//! `dist/game/data/scripts/quests/Q00417_PathOfTheScavenger/`. At 690 Java
//! lines the largest quest in the Path family, and the **last** of it.
//!
//! Awards the **Ring of Raven** (1642). With this, all four races'
//! first-occupation scripts are proof-complete.
//!
//! Pipi's recommendation → Mion's courier rounds → Toma's bear and tarantula
//! commissions → Raut's parcel → Torai → the ring.
//!
//! ## `dropChance` is documented as 0..1, and this quest passes 50
//!
//! ```java
//! giveItemRandomly(killer, npc, HONEY_JAR, 1, 5, 50, true)
//! ```
//!
//! `AbstractScript.giveItemRandomly`'s javadoc is explicit — *"the drop chance
//! as a decimal digit from 0 to 1"* — so 50 is not 50%, it is fifty times
//! certainty: **every qualifying kill drops.** Compare `q00303`, which passes
//! `0.4` for a real 40%.
//!
//! This is a datapack bug, and per the repo's rule the dist is authoritative,
//! so the port passes `50.0` and drops on every kill exactly as the live
//! server does. Writing the "obviously intended" `0.5` would halve the drop
//! rate against retail — a silent divergence in the direction that looks
//! correct. Tested by killing once and asserting a drop with no forced roll.
//!
//! ## Two spoil-gated payouts — the Scavenger's own mechanic
//!
//! Honey jars and beads pay only when the corpse `isSpoiled()`, and `onAttack`
//! additionally disqualifies a mob whose spoiler is the attacker
//! (`getSpoilerObjectId() == attacker` → script value 2). So the quest wants
//! you to Spoil, and the first-attacker tag runs alongside it. Both use
//! `FIRST_ATTACKER` as the npc variable — a **fourth** spelling after
//! `lastAttacker`, `firstAttacker` and `Q00415_last_attacker`.
//!
//! ## Three counters in one integer
//!
//! `memoStateEx(1)` packs two counters by radix: **+10 per delivery**
//! (tens digit) and **+1 per Mion dialogue step** (units), read back with
//! `% 10` for the units and `< 20` / `< 50` thresholds for the tens. Treating
//! it as a single counter breaks both halves.
//!
//! Alongside it, `FLAG` is an escalating summon meter for the Honey Bear:
//! each ordinary Hunter Bear kill raises it, and the spawn chance is
//! `20 * flag` percent, resetting to 0 on success. That is the **third**
//! distinct summon-meter shape in this family after 414's green blood and
//! 416's Durka parasites.
//!
//! ## Dead at both ends — fifth quest running
//!
//! NPC **31958** ships pages and is registered nowhere, and the
//! `BEAD_PARCEL2` / `memoState 2` route that reaches it (`30556-06b`) is
//! offered by no page. Omitted, as in 416 and 418.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const WAREHOUSE_KEEPER_RAUT: i32 = 30316;
const TRADER_SHARI: i32 = 30517;
const TRADER_MION: i32 = 30519;
const COLLECTOR_PIPI: i32 = 30524;
const HEAD_BLACKSMITH_BRONK: i32 = 30525;
const PRIEST_OF_THE_EARTH_ZIMENF: i32 = 30538;
const MASTER_TOMA: i32 = 30556;
const TORAI: i32 = 30557;

const RING_OF_RAVEN: i32 = 1642;
const PIPPIS_LETTER: i32 = 1643;
const ROUTS_TELEPORT_SCROLL: i32 = 1644;
const SUCCUBUS_UNDIES: i32 = 1645;
const MIONS_LETTER: i32 = 1646;
const BRONKS_INGOT: i32 = 1647;
const SHARIS_AXE: i32 = 1648;
const ZIMENFS_POTION: i32 = 1649;
const BRONKS_PAY: i32 = 1650;
const SHARIS_PAY: i32 = 1651;
const ZIMENFS_PAY: i32 = 1652;
const BEAR_PICTURE: i32 = 1653;
const TARANTULA_PICTURE: i32 = 1654;
const HONEY_JAR: i32 = 1655;
const BEAD: i32 = 1656;
const BEAD_PARCEL: i32 = 1657;

const HUNTER_TARANTULA: i32 = 20403;
const PLUNDER_TARANTULA: i32 = 20508;
const HUNTER_BEAR: i32 = 20777;
const HONEY_BEAR: i32 = 27058;

const DWARVEN_FIGHTER: i32 = 53;
const SCAVENGER: i32 = 54;
const MIN_LEVEL: i32 = 19;

const HONEY_NEEDED: i64 = 5;
const BEADS_NEEDED: i64 = 20;

/// Java's npc-variable key — a fourth spelling in this family.
const FIRST_ATTACKER: &str = "FIRST_ATTACKER";
/// Java's *quest*-variable key for the Honey Bear summon meter.
const FLAG: &str = "FLAG";

/// The three errands Mion hands out at random: `(delivery item, pay item,
/// page)`.
const ERRANDS: [(i32, i32, &str); 3] = [
    (ZIMENFS_POTION, ZIMENFS_PAY, "30519-02.html"),
    (SHARIS_AXE, SHARIS_PAY, "30519-03.html"),
    (BRONKS_INGOT, BRONKS_PAY, "30519-04.html"),
];

/// `(npc, delivery item, pay item, first page, promoted page)`.
const DELIVERIES: [(i32, i32, i32, &str, &str); 3] = [
    (
        PRIEST_OF_THE_EARTH_ZIMENF,
        ZIMENFS_POTION,
        ZIMENFS_PAY,
        "30538-01.html",
        "30538-02.html",
    ),
    (
        TRADER_SHARI,
        SHARIS_AXE,
        SHARIS_PAY,
        "30517-01.html",
        "30517-02.html",
    ),
    (
        HEAD_BLACKSMITH_BRONK,
        BRONKS_INGOT,
        BRONKS_PAY,
        "30525-01.html",
        "30525-02.html",
    ),
];

const TAGGED_MOBS: [i32; 4] = [HUNTER_TARANTULA, PLUNDER_TARANTULA, HUNTER_BEAR, HONEY_BEAR];

const QUEST_ITEMS: [i32; 15] = [
    PIPPIS_LETTER,
    ROUTS_TELEPORT_SCROLL,
    SUCCUBUS_UNDIES,
    MIONS_LETTER,
    BRONKS_INGOT,
    SHARIS_AXE,
    ZIMENFS_POTION,
    BRONKS_PAY,
    SHARIS_PAY,
    ZIMENFS_PAY,
    BEAR_PICTURE,
    TARANTULA_PICTURE,
    HONEY_JAR,
    BEAD,
    BEAD_PARCEL,
];

pub struct Q00417PathOfTheScavenger;

impl Q00417PathOfTheScavenger {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    fn has_any(&self, ctx: &QuestCtx, items: &[i32]) -> bool {
        items.iter().any(|id| ctx.quest_items_count(*id) > 0)
    }
    fn pays_held(&self, ctx: &QuestCtx) -> i64 {
        ctx.quest_items_count(SHARIS_PAY)
            + ctx.quest_items_count(BRONKS_PAY)
            + ctx.quest_items_count(ZIMENFS_PAY)
    }
    fn deliveries_held(&self, ctx: &QuestCtx) -> i64 {
        ctx.quest_items_count(SHARIS_AXE)
            + ctx.quest_items_count(BRONKS_INGOT)
            + ctx.quest_items_count(ZIMENFS_POTION)
    }
    /// Hand out one of the three errands at random.
    fn give_random_errand(&self, ctx: &mut QuestCtx) -> String {
        let (item, _, page) = ERRANDS[ctx.roll(3) as usize];
        ctx.give_items(item, 1);
        page.to_string()
    }
    /// Only the player who struck first is paid.
    fn is_first_attacker(&self, ctx: &QuestCtx) -> bool {
        ctx.npc_var_int(FIRST_ATTACKER) == ctx.player
    }
}

impl QuestScript for Q00417PathOfTheScavenger {
    fn id(&self) -> i32 {
        417
    }
    fn name(&self) -> &'static str {
        "Q00417_PathOfTheScavenger"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00417_PathOfTheScavenger"
    }
    fn start_npcs(&self) -> &[i32] {
        &[COLLECTOR_PIPI]
    }
    /// 31958 is deliberately absent — see the module header.
    fn talk_npcs(&self) -> &[i32] {
        &[
            COLLECTOR_PIPI,
            WAREHOUSE_KEEPER_RAUT,
            TRADER_MION,
            TRADER_SHARI,
            HEAD_BLACKSMITH_BRONK,
            PRIEST_OF_THE_EARTH_ZIMENF,
            MASTER_TOMA,
            TORAI,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &TAGGED_MOBS
    }
    fn attack_npcs(&self) -> &[i32] {
        &TAGGED_MOBS
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(match ctx.player_class_id() {
                DWARVEN_FIGHTER if ctx.player_level() < MIN_LEVEL => "30524-02.htm".to_string(),
                DWARVEN_FIGHTER if self.has(ctx, RING_OF_RAVEN) => "30524-04.htm".to_string(),
                DWARVEN_FIGHTER => {
                    ctx.start_quest();
                    ctx.set_memo_state_ex(1, 0);
                    ctx.give_items(PIPPIS_LETTER, 1);
                    "30524-05.htm".to_string()
                }
                SCAVENGER => "30524-02a.htm".to_string(),
                _ => "30524-08.htm".to_string(),
            }),
            "30524-03.html" | "30557-02.html" | "30519-06.html" => Some(event.to_string()),
            // Pipi's letter buys the first errand.
            "reply_1" => {
                if self.has(ctx, PIPPIS_LETTER) {
                    ctx.take_items(PIPPIS_LETTER, 1);
                    return Some(self.give_random_errand(ctx));
                }
                None
            }
            // Trading a full set of pays for the next errand.
            "reply_4" => {
                ctx.take_items(ZIMENFS_PAY, 1);
                ctx.take_items(SHARIS_PAY, 1);
                ctx.take_items(BRONKS_PAY, 1);
                Some(self.give_random_errand(ctx))
            }
            "reply_2" => Some(
                if ctx.roll(2) == 0 {
                    "30519-06.html"
                } else {
                    "30519-11.html"
                }
                .to_string(),
            ),
            "reply_3" => {
                // The units digit of memoStateEx(1) counts Mion's dialogue.
                let ex = ctx.memo_state_ex(1);
                let units = ex % 10;
                let memo = ctx.memo_state();
                if units < 2 {
                    ctx.set_memo_state_ex(1, ex + 1);
                    return Some("30519-07.html".to_string());
                }
                if units == 2 && memo == 0 {
                    return Some("30519-07.html".to_string());
                }
                if units == 2 && memo == 1 {
                    ctx.set_memo_state_ex(1, ex + 1);
                    return Some("30519-09.html".to_string());
                }
                None
            }
            "30519-07.html" => {
                let ex = ctx.memo_state_ex(1);
                ctx.set_memo_state_ex(1, ex + 1);
                Some(event.to_string())
            }
            // Toma packs the beads.
            "30556-05b.html" => {
                if self.has(ctx, TARANTULA_PICTURE) && ctx.quest_items_count(BEAD) >= BEADS_NEEDED {
                    ctx.take_items(TARANTULA_PICTURE, 1);
                    ctx.take_items(BEAD, -1);
                    ctx.give_items(BEAD_PARCEL, 1);
                    ctx.set_cond(9, true);
                    return Some(event.to_string());
                }
                None
            }
            "30316-02.html" | "30316-03.html" => {
                if self.has(ctx, BEAD_PARCEL) {
                    ctx.take_items(BEAD_PARCEL, 1);
                    ctx.give_items(ROUTS_TELEPORT_SCROLL, 1);
                    ctx.set_cond(10, true);
                    return Some(event.to_string());
                }
                None
            }
            // Torai takes the scroll and vanishes.
            "30557-03.html" => {
                if self.has(ctx, ROUTS_TELEPORT_SCROLL) {
                    ctx.take_items(ROUTS_TELEPORT_SCROLL, 1);
                    ctx.give_items(SUCCUBUS_UNDIES, 1);
                    ctx.set_cond(11, true);
                    ctx.delete_npc(); // `npc.deleteMe()`
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    /// First-attacker tag, plus the Scavenger twist: spoiling the mob yourself
    /// disqualifies it (`getSpoilerObjectId() == attacker` → value 2).
    fn on_attack(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let player = ctx.player;
        if ctx.npc_id == HUNTER_BEAR {
            match ctx.npc_script_value() {
                0 => {
                    ctx.set_npc_script_value(1);
                    ctx.set_npc_var_int(FIRST_ATTACKER, player);
                }
                1 if ctx.npc_var_int(FIRST_ATTACKER) != player => ctx.set_npc_script_value(2),
                _ => {}
            }
            return;
        }
        if ctx.npc_script_value() == 0 {
            ctx.set_npc_script_value(1);
            ctx.set_npc_var_int(FIRST_ATTACKER, player);
        }
        if ctx.npc_spoiler_object_id() == player {
            ctx.set_npc_script_value(2);
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() || !self.is_first_attacker(ctx) {
            return;
        }
        match ctx.npc_id {
            HUNTER_BEAR => {
                if !self.has(ctx, BEAR_PICTURE) || ctx.quest_items_count(HONEY_JAR) >= HONEY_NEEDED
                {
                    return;
                }
                // The summon meter: `20 * flag` percent, reset on success.
                let flag = ctx.get_int(FLAG);
                if flag > 0 && ctx.roll(100) < 20 * flag {
                    ctx.spawn_attacker(HONEY_BEAR, true);
                    ctx.set_var(FLAG, "0");
                } else {
                    ctx.set_var(FLAG, (flag + 1).to_string());
                }
            }
            // Both payouts want a spoiled corpse and the matching commission.
            // The `50.0` is Java's — the API wants a 0..1 fraction, so this
            // always drops. See the module header.
            HONEY_BEAR => {
                let eligible = ctx.npc_is_spoiled() && self.has(ctx, BEAR_PICTURE);
                if eligible && ctx.give_item_randomly(HONEY_JAR, 1, HONEY_NEEDED, 50.0, true) {
                    ctx.set_cond(6, false);
                }
            }
            HUNTER_TARANTULA | PLUNDER_TARANTULA => {
                let eligible = ctx.npc_is_spoiled() && self.has(ctx, TARANTULA_PICTURE);
                if eligible && ctx.give_item_randomly(BEAD, 1, BEADS_NEEDED, 50.0, true) {
                    ctx.set_cond(8, false);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == COLLECTOR_PIPI {
                return Some("30524-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            COLLECTOR_PIPI => Some(
                if self.has(ctx, PIPPIS_LETTER) {
                    "30524-06.html"
                } else {
                    "30524-07.html"
                }
                .to_string(),
            ),
            TRADER_MION => self.talk_mion(ctx),
            MASTER_TOMA => self.talk_toma(ctx),
            WAREHOUSE_KEEPER_RAUT => self.talk_raut(ctx),
            TORAI => Some(if self.has(ctx, ROUTS_TELEPORT_SCROLL) {
                "30557-01.html".to_string()
            } else {
                ctx.no_quest_html()
            }),
            _ => self.talk_delivery(ctx, npc),
        }
    }
}

impl Q00417PathOfTheScavenger {
    fn talk_mion(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, PIPPIS_LETTER) {
            ctx.set_cond(2, true);
            return Some("30519-01.html".to_string());
        }
        if self.deliveries_held(ctx) == 1 {
            return Some(
                if ctx.memo_state_ex(1) % 10 == 0 {
                    "30519-05.html"
                } else {
                    "30519-08.html"
                }
                .to_string(),
            );
        }
        if self.pays_held(ctx) == 1 {
            // Tens digit: five deliveries done.
            if ctx.memo_state_ex(1) < 50 {
                return Some("30519-12.html".to_string());
            }
            ctx.give_items(MIONS_LETTER, 1);
            ctx.take_items(SHARIS_PAY, 1);
            ctx.take_items(ZIMENFS_PAY, 1);
            ctx.take_items(BRONKS_PAY, 1);
            ctx.set_cond(4, true);
            return Some("30519-15.html".to_string());
        }
        if self.has(ctx, MIONS_LETTER) {
            return Some("30519-13.html".to_string());
        }
        if self.has_any(
            ctx,
            &[
                BEAR_PICTURE,
                TARANTULA_PICTURE,
                BEAD_PARCEL,
                ROUTS_TELEPORT_SCROLL,
                SUCCUBUS_UNDIES,
            ],
        ) {
            return Some("30519-14.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    /// Shari, Bronk and Zimenf share one shape: take the delivery, pay, and
    /// bump the tens digit. The second hand-in promotes the quest to cond 3.
    fn talk_delivery(&self, ctx: &mut QuestCtx, npc: i32) -> Option<String> {
        let Some(&(_, item, pay, first, promoted)) = DELIVERIES.iter().find(|(id, ..)| *id == npc)
        else {
            return Some(ctx.no_quest_html());
        };
        if self.has(ctx, item) {
            let ex = ctx.memo_state_ex(1);
            ctx.take_items(item, 1);
            ctx.give_items(pay, 1);
            ctx.set_memo_state_ex(1, ex + 10);
            if ex < 20 {
                return Some(first.to_string());
            }
            ctx.set_memo_state(1);
            ctx.set_cond(3, true);
            return Some(promoted.to_string());
        }
        if self.has(ctx, pay) {
            // Each giver's third page is its "already paid" line.
            let waiting = match npc {
                TRADER_SHARI => "30517-03.html",
                HEAD_BLACKSMITH_BRONK => "30525-03.html",
                _ => "30538-03.html",
            };
            return Some(waiting.to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_toma(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, MIONS_LETTER) {
            ctx.take_items(MIONS_LETTER, 1);
            ctx.give_items(BEAR_PICTURE, 1);
            ctx.set_cond(5, true);
            ctx.set_var(FLAG, "0");
            return Some("30556-01.html".to_string());
        }
        if self.has(ctx, BEAR_PICTURE) {
            if ctx.quest_items_count(HONEY_JAR) < HONEY_NEEDED {
                return Some("30556-02.html".to_string());
            }
            ctx.take_items(BEAR_PICTURE, 1);
            ctx.give_items(TARANTULA_PICTURE, 1);
            ctx.take_items(HONEY_JAR, -1);
            ctx.set_cond(7, true);
            return Some("30556-03.html".to_string());
        }
        if self.has(ctx, TARANTULA_PICTURE) {
            return Some(
                if ctx.quest_items_count(BEAD) < BEADS_NEEDED {
                    "30556-04.html"
                } else {
                    "30556-05a.html"
                }
                .to_string(),
            );
        }
        if self.has(ctx, BEAD_PARCEL) {
            return Some("30556-06a.html".to_string());
        }
        if self.has_any(ctx, &[ROUTS_TELEPORT_SCROLL, SUCCUBUS_UNDIES]) {
            return Some("30556-07.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_raut(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, BEAD_PARCEL) {
            return Some("30316-01.html".to_string());
        }
        if self.has(ctx, ROUTS_TELEPORT_SCROLL) {
            return Some("30316-04.html".to_string());
        }
        if self.has(ctx, SUCCUBUS_UNDIES) {
            ctx.give_items(RING_OF_RAVEN, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30316-05.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
