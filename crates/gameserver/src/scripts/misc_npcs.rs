//! The `ai/others` tail — the small talk/utility NPCs that belong to no larger
//! system:
//!
//! - `ArenaManager` — the Coliseum/MDT attendants' paid CP/HP recovery and buff
//!   package.
//! - `ToIVortex` — the Tower of Insolence floor teleports and the dimension
//!   stone trade.
//! - `SymbolMaker` — the dye NPCs' chat window (their `Draw`/`Remove` buttons
//!   were already wired to `game_loop::character::henna`; without this the window that
//!   holds the buttons never opened).
//! - `RandomWalkingGuards` — the five village guards Java lets wander even
//!   though `Guard`s have random walking off by default.

use crate::data::item_data::ADENA_ID;
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::game_loop::skills::skill_by_id;
// ---------------------------------------------------------------------------
// ArenaManager
// ---------------------------------------------------------------------------

pub struct ArenaManager;

const ARENA_MANAGERS: &[i32] = &[
    31225, // Arena Director (Monster Derby Track)
    31226, // Arena Manager (Coliseum)
];

/// Java's `BUFFS` — the six arena buffs, all level 1.
const ARENA_BUFFS: &[i32] = &[
    6805, // Arena Empower
    6806, // Arena Acumen
    6807, // Arena Concentration
    6808, // Arena Might
    6804, // Arena Wind Walk
    6812, // Arena Berserker Spirit
];
/// `CP_RECOVERY` / `HP_RECOVERY`.
const CP_RECOVERY: i32 = 4380;
const HP_RECOVERY: i32 = 6817;

const CP_COST: i64 = 1000;
const HP_COST: i64 = 1000;
const BUFF_COST: i64 = 2000;
/// Java's `startQuestTimer(…, 2000, …)` before the recovery cast.
const RECOVERY_DELAY_MS: u64 = 2000;

impl QuestScript for ArenaManager {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ArenaManager"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/ArenaManager"
    }
    fn start_npcs(&self) -> &[i32] {
        ARENA_MANAGERS
    }
    fn talk_npcs(&self) -> &[i32] {
        ARENA_MANAGERS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        ARENA_MANAGERS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `AbstractNpcAI.onFirstTalk` — `<npcId>.html`.
    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        Some(format!("{}.html", ctx.npc_id))
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            // Both recoveries are paid up front and cast two seconds later, so
            // the attendant can't be milked by walking off mid-cast.
            "CPrecovery" | "HPrecovery" => {
                let cost = if event == "CPrecovery" {
                    CP_COST
                } else {
                    HP_COST
                };
                if ctx.quest_items_count(ADENA_ID) < cost {
                    send_no_adena(ctx);
                    return None;
                }
                ctx.take_items(ADENA_ID, cost);
                ctx.start_quest_timer(&format!("{event}_delay"), RECOVERY_DELAY_MS);
                None
            }
            "Buff" => {
                if ctx.quest_items_count(ADENA_ID) < BUFF_COST {
                    send_no_adena(ctx);
                    return None;
                }
                ctx.take_items(ADENA_ID, BUFF_COST);
                for &skill_id in ARENA_BUFFS {
                    trigger_cast(ctx, skill_id);
                }
                None
            }
            // The delayed halves: Java re-checks that the buyer hasn't stepped
            // into a PVP zone (an arena) in the meantime.
            "CPrecovery_delay" | "HPrecovery_delay" => {
                if in_pvp_zone(ctx) {
                    return None;
                }
                let skill_id = if event.starts_with("CP") {
                    CP_RECOVERY
                } else {
                    HP_RECOVERY
                };
                trigger_cast(ctx, skill_id);
                None
            }
            _ => None,
        }
    }
}

/// `SystemMessageId.YOU_DO_NOT_HAVE_ENOUGH_ADENA`.
fn send_no_adena(ctx: &QuestCtx) {
    crate::game_loop::helpers::send_sm_bare_to_client(
        ctx.world,
        ctx.client_id,
        crate::network::server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA,
    );
}

/// `SkillCaster.triggerCast(npc, player, skill)` / `npc.doCast` — the NPC casts
/// on the player with no cast time.
fn trigger_cast(ctx: &mut QuestCtx, skill_id: i32) {
    let Some(skill) = skill_by_id(ctx.world, skill_id, 1) else {
        return;
    };
    crate::game_loop::npc::cast::start_cast(ctx.world, ctx.npc, ctx.player, &skill);
}

/// `player.isInsideZone(ZoneId.PVP)`.
fn in_pvp_zone(ctx: &QuestCtx) -> bool {
    let Some(pos) = ctx
        .world
        .objects
        .get_component::<crate::model::components::space::Position>(&ctx.player)
    else {
        return false;
    };
    let (x, y, z) = (pos.x, pos.y, pos.z);
    ctx.world
        .data
        .zone_data
        .zones_at(x, y, z)
        .any(|zn| zn.kind == crate::data::zone_data::ZoneKind::Pvp)
}

// ---------------------------------------------------------------------------
// ToIVortex
// ---------------------------------------------------------------------------

pub struct ToIVortex;

const TOI_NPCS: &[i32] = &[
    30949, // Keplon
    30950, // Euclie
    30951, // Pithgon
    30952, // Dimension Vortex 1
    30953, // Dimension Vortex 2
    30954, // Dimension Vortex 3
];

/// Dimension stones (Java flags a `4401` mismatch and keeps these anyway).
const GREEN_DIMENSION_STONE: i32 = 4404;
const BLUE_DIMENSION_STONE: i32 = 4405;
const RED_DIMENSION_STONE: i32 = 4406;
/// What one stone costs at the trade counter.
const STONE_PRICE: i64 = 100_000;

/// `TOI_FLOORS` + `TOI_FLOOR_ITEMS`: floor → destination and the stone it eats.
const TOI_FLOORS: &[(&str, (i32, i32, i32), i32)] = &[
    ("1", (114356, 13423, -5096), GREEN_DIMENSION_STONE),
    ("2", (114666, 13380, -3608), GREEN_DIMENSION_STONE),
    ("3", (111982, 16028, -2120), GREEN_DIMENSION_STONE),
    ("4", (114636, 13413, -640), BLUE_DIMENSION_STONE),
    ("5", (114152, 19902, 928), BLUE_DIMENSION_STONE),
    ("6", (117131, 16044, 1944), BLUE_DIMENSION_STONE),
    ("7", (113026, 17687, 2952), RED_DIMENSION_STONE),
    ("8", (115571, 13723, 3960), RED_DIMENSION_STONE),
    ("9", (114649, 14144, 4976), RED_DIMENSION_STONE),
    ("10", (118507, 16605, 5984), RED_DIMENSION_STONE),
];

impl QuestScript for ToIVortex {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ToIVortex"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/ToIVortex"
    }
    fn start_npcs(&self) -> &[i32] {
        TOI_NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        TOI_NPCS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // A floor number: one stone of the floor's colour buys the ride.
        if let Some(&(_, loc, item_id)) = TOI_FLOORS.iter().find(|(name, _, _)| *name == event) {
            if ctx.quest_items_count(item_id) == 0 {
                return Some("no-stones.htm".to_string());
            }
            ctx.take_items(item_id, 1);
            ctx.teleport_to(loc.0, loc.1, loc.2);
            return None;
        }
        // The trade counter: 100k adena for one stone of the named colour.
        let stone = match event {
            "GREEN" => GREEN_DIMENSION_STONE,
            "BLUE" => BLUE_DIMENSION_STONE,
            "RED" => RED_DIMENSION_STONE,
            _ => return None,
        };
        if ctx.quest_items_count(ADENA_ID) < STONE_PRICE {
            // Java builds this page name from the npc id — each of the three
            // sellers has its own refusal html.
            return Some(format!("{}no-adena.htm", ctx.npc_id));
        }
        ctx.take_items(ADENA_ID, STONE_PRICE);
        ctx.give_items(stone, 1);
        None
    }
}

// ---------------------------------------------------------------------------
// SymbolMaker
// ---------------------------------------------------------------------------

pub struct SymbolMaker;

const SYMBOL_MAKERS: &[i32] = &[
    31046, 31047, 31048, 31049, 31050, 31051, 31052, 31053, 31264, 31308, 31953,
];

/// The pages the window navigates between (Java's pass-through `case`s). The
/// `Draw`/`Remove` buttons on them are handled by the bypass router
/// (`game_loop::character::henna`).
const SYMBOL_PAGES: &[&str] = &[
    "symbol_maker.htm",
    "symbol_maker-1.htm",
    "symbol_maker-2.htm",
    "symbol_maker-3.htm",
];

impl QuestScript for SymbolMaker {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "SymbolMaker"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/SymbolMaker"
    }
    fn start_npcs(&self) -> &[i32] {
        SYMBOL_MAKERS
    }
    fn talk_npcs(&self) -> &[i32] {
        SYMBOL_MAKERS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        SYMBOL_MAKERS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_first_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some("symbol_maker.htm".to_string())
    }

    fn on_event(&self, _ctx: &mut QuestCtx, event: &str) -> Option<String> {
        SYMBOL_PAGES.contains(&event).then(|| event.to_string())
    }
}

// ---------------------------------------------------------------------------
// RandomWalkingGuards
// ---------------------------------------------------------------------------

pub struct RandomWalkingGuards;

/// The five village guards Java sets wandering. `Guard`-type NPCs have random
/// walking off by default (`Npc.isRandomWalkingEnabled`), which is exactly why
/// the script exists.
pub(crate) const WALKING_GUARDS: &[i32] = &[
    31032, // Talking Island
    31033, // Elven Village
    31034, // Dark Elf Village
    31036, // Orc Village
    31035, // Dwarven Village
];

impl QuestScript for RandomWalkingGuards {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "RandomWalkingGuards"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn spawn_npcs(&self) -> &[i32] {
        WALKING_GUARDS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `onSpawn`: arm the first stroll. The beat itself is a plain scheduled
    /// task (`game_loop::area_npcs`) — Java's timer carries an NPC and a null
    /// player, which the player-anchored quest timers here cannot express.
    fn on_spawn(&self, ctx: &mut QuestCtx) {
        crate::game_loop::npc::area::arm_guard_walk(ctx.world, ctx.npc);
    }
}
