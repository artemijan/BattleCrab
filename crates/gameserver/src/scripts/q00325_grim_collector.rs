//! Grim Collector (325) — `quests/Q00325_GrimCollector`. Guard Curtiz (30336,
//! level 15+) starts a grave-robbing errand: Samed (30434) hands out an Anatomy
//! Diagram, undead then drop organs/bones on a per-mob cumulative-threshold
//! ladder, Varsak (30342) assembles five bones into a Complete Skeleton (80%),
//! and Samed buys the lot.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const GUARD_CURTIZ: i32 = 30336;
const VARSAK: i32 = 30342;
const SAMED: i32 = 30434;
const ANATOMY_DIAGRAM: i32 = 1349;
const ZOMBIE_HEAD: i32 = 1350;
const ZOMBIE_HEART: i32 = 1351;
const ZOMBIE_LIVER: i32 = 1352;
const SKULL: i32 = 1353;
const RIB_BONE: i32 = 1354;
const SPINE: i32 = 1355;
const ARM_BONE: i32 = 1356;
const THIGH_BONE: i32 = 1357;
const COMPLETE_SKELETON: i32 = 1358;
const MIN_LEVEL: i32 = 15;
const REGISTERED: [i32; 10] = [
    ANATOMY_DIAGRAM,
    ZOMBIE_HEAD,
    ZOMBIE_HEART,
    ZOMBIE_LIVER,
    SKULL,
    RIB_BONE,
    SPINE,
    ARM_BONE,
    THIGH_BONE,
    COMPLETE_SKELETON,
];
/// The five bones Varsak assembles into a Complete Skeleton.
const BONES: [i32; 5] = [SPINE, ARM_BONE, SKULL, RIB_BONE, THIGH_BONE];

/// `MONSTER_DROPS`: per-mob ladder of (item, cumulative upper-bound out of 100).
/// The kill rolls `getRandom(100)` and drops the first entry whose bound it is
/// under (nothing if it clears them all).
fn drops_for(npc_id: i32) -> &'static [(i32, i32)] {
    match npc_id {
        20026 => &[(ZOMBIE_HEAD, 30), (ZOMBIE_HEART, 50), (ZOMBIE_LIVER, 75)],
        20029 => &[(ZOMBIE_HEAD, 30), (ZOMBIE_HEART, 52), (ZOMBIE_LIVER, 75)],
        20035 => &[(SKULL, 5), (RIB_BONE, 15), (SPINE, 29), (THIGH_BONE, 79)],
        20042 => &[(SKULL, 6), (RIB_BONE, 19), (ARM_BONE, 69), (THIGH_BONE, 86)],
        20045 => &[(SKULL, 9), (SPINE, 59), (ARM_BONE, 77), (THIGH_BONE, 97)],
        20051 => &[(SKULL, 9), (RIB_BONE, 59), (SPINE, 79), (ARM_BONE, 100)],
        20457 => &[(ZOMBIE_HEAD, 40), (ZOMBIE_HEART, 60), (ZOMBIE_LIVER, 80)],
        20458 => &[(ZOMBIE_HEAD, 40), (ZOMBIE_HEART, 70), (ZOMBIE_LIVER, 100)],
        20514 => &[
            (SKULL, 6),
            (RIB_BONE, 21),
            (SPINE, 30),
            (ARM_BONE, 31),
            (THIGH_BONE, 64),
        ],
        20515 => &[
            (SKULL, 5),
            (RIB_BONE, 20),
            (SPINE, 31),
            (ARM_BONE, 33),
            (THIGH_BONE, 69),
        ],
        _ => &[],
    }
}

fn has_all_registered(ctx: &QuestCtx) -> bool {
    REGISTERED.iter().all(|&id| ctx.quest_items_count(id) > 0)
}
fn has_any_registered(ctx: &QuestCtx) -> bool {
    REGISTERED.iter().any(|&id| ctx.quest_items_count(id) > 0)
}

pub struct Q00325GrimCollector;

impl QuestScript for Q00325GrimCollector {
    fn id(&self) -> i32 {
        325
    }
    fn name(&self) -> &'static str {
        "Q00325_GrimCollector"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00325_GrimCollector"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GUARD_CURTIZ]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[GUARD_CURTIZ, VARSAK, SAMED]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            20026, 20029, 20035, 20042, 20045, 20051, 20457, 20458, 20514, 20515,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &REGISTERED
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30336-03.htm" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "assembleSkeleton" => {
                if !BONES.iter().all(|&id| ctx.quest_items_count(id) > 0) {
                    Some("30342-02.html".to_string())
                } else {
                    for id in BONES {
                        ctx.take_items(id, 1);
                    }
                    if ctx.roll(5) < 4 {
                        ctx.give_items(COMPLETE_SKELETON, 1);
                        Some("30342-03.html".to_string())
                    } else {
                        Some("30342-04.html".to_string())
                    }
                }
            }
            "30434-02.htm" => Some(event.to_string()),
            "30434-03.html" => {
                ctx.give_items(ANATOMY_DIAGRAM, 1);
                Some(event.to_string())
            }
            "30434-06.html" | "30434-07.html" => {
                // Java's `hasQuestItems(getRegisteredItemIds())` requires ALL ten
                // registered items (diagram + every part + a complete skeleton) —
                // a demanding but shipped gate; the pay/take only fire then.
                if has_all_registered(ctx) {
                    let head = ctx.quest_items_count(ZOMBIE_HEAD);
                    let heart = ctx.quest_items_count(ZOMBIE_HEART);
                    let liver = ctx.quest_items_count(ZOMBIE_LIVER);
                    let skull = ctx.quest_items_count(SKULL);
                    let rib = ctx.quest_items_count(RIB_BONE);
                    let spine = ctx.quest_items_count(SPINE);
                    let arm = ctx.quest_items_count(ARM_BONE);
                    let thigh = ctx.quest_items_count(THIGH_BONE);
                    let complete = ctx.quest_items_count(COMPLETE_SKELETON);
                    let total = head + heart + liver + skull + rib + spine + arm + thigh + complete;
                    if total > 0 {
                        let mut sum = head * 8
                            + heart * 5
                            + liver * 5
                            + skull * 25
                            + rib * 5
                            + spine * 5
                            + arm * 5
                            + thigh * 5;
                        if total >= 10 {
                            sum += 1629;
                        }
                        if complete > 0 {
                            sum += 543 + complete * 341;
                        }
                        ctx.give_adena(sum, true);
                    }
                    for id in REGISTERED {
                        ctx.take_items(id, -1);
                    }
                }
                if event == "30434-06.html" {
                    ctx.exit_quest(true, true);
                }
                Some(event.to_string())
            }
            "30434-09.html" => {
                // Sell only assembled skeletons. Java sets no html here (returns
                // null), so no page is shown.
                let complete = ctx.quest_items_count(COMPLETE_SKELETON);
                if complete > 0 {
                    ctx.give_adena(complete * 341 + 543, true);
                    ctx.take_items(COMPLETE_SKELETON, -1);
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `Util.checkIfInRange` is trivially true for the killer; the diagram is
        // the real gate. Port is killer-only (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_started() || ctx.quest_items_count(ANATOMY_DIAGRAM) == 0 {
            return;
        }
        let rnd = ctx.roll(100);
        for &(item, chance) in drops_for(ctx.npc_id) {
            if rnd < chance {
                ctx.give_item_randomly(item, 1, 0, 1.0, true);
                break;
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            GUARD_CURTIZ => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_level() >= MIN_LEVEL {
                            "30336-02.htm"
                        } else {
                            "30336-01.htm"
                        }
                        .to_string(),
                    );
                }
                if ctx.is_started() {
                    return Some(
                        if ctx.quest_items_count(ANATOMY_DIAGRAM) > 0 {
                            "30336-05.html"
                        } else {
                            "30336-04.html"
                        }
                        .to_string(),
                    );
                }
                Some(ctx.no_quest_html())
            }
            VARSAK => {
                if ctx.is_started() && ctx.quest_items_count(ANATOMY_DIAGRAM) > 0 {
                    return Some("30342-01.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            SAMED => {
                if ctx.is_started() {
                    if ctx.quest_items_count(ANATOMY_DIAGRAM) == 0 {
                        return Some("30434-01.html".to_string());
                    }
                    // `hasAtLeastOneQuestItem(registered)` — the diagram is
                    // registered, so this is always true here; 30434-04 is
                    // effectively unreachable (kept as Java has it).
                    if !has_any_registered(ctx) {
                        return Some("30434-04.html".to_string());
                    }
                    if ctx.quest_items_count(COMPLETE_SKELETON) == 0 {
                        return Some("30434-05.html".to_string());
                    }
                    return Some("30434-08.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            _ => Some(ctx.no_quest_html()),
        }
    }
}
