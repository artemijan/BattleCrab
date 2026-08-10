//! `ai/areas/KetraOrcOutpust/KetraOrcSupport` +
//! `ai/areas/VarkaSilenosBarracks/VarkaSilenosSupport` — the allied tribes'
//! service NPCs, a mirror pair on one engine (like the mirror quests):
//! same seven roles, same eight-buff price list, different ids/items/marks.
//!
//! A player's standing is the highest alliance-mark item in their bag
//! (level 1–5); every chat window keys off it, the buffer trades
//! horns/seeds for war buffs, and the teleporter serves level 4+.

use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::quests::{QuestCtx, QuestScript};

/// `BUFF` — event digit → (skill, cost in horns/seeds). Same for both tribes.
const BUFFS: [(i32, i64); 8] = [
    (4359, 2), // 1: Focus
    (4360, 2), // 2: Death Whisper
    (4345, 3), // 3: Might
    (4355, 3), // 4: Acumen
    (4352, 3), // 5: Berserker
    (4354, 3), // 6: Vampiric Rage
    (4356, 6), // 7: Empower
    (4357, 6), // 8: Haste
];

struct Tribe {
    name: &'static str,
    html_dir: &'static str,
    /// Hierarch, Messenger, Buffer, Grocer, Warehouse, Trader, Teleporter.
    hierarch: i32,
    messenger: i32,
    buffer: i32,
    grocer: i32,
    warehouse: i32,
    trader: i32,
    teleporter: i32,
    /// Buffalo Horn / Nepenthese Seed.
    currency: i32,
    /// The five alliance marks, level 1..=5.
    marks: [i32; 5],
}

const KETRA: Tribe = Tribe {
    name: "KetraOrcSupport",
    html_dir: "ai/areas/KetraOrcOutpust/KetraOrcSupport",
    hierarch: 31370,   // Kadun
    messenger: 31371,  // Wahkan
    buffer: 31372,     // Asefa
    grocer: 31373,     // Atan
    warehouse: 31374,  // Jaff
    trader: 31375,     // Jumara
    teleporter: 31376, // Kurfa
    currency: 7186,    // Buffalo Horn
    marks: [7211, 7212, 7213, 7214, 7215],
};

const VARKA: Tribe = Tribe {
    name: "VarkaSilenosSupport",
    html_dir: "ai/areas/VarkaSilenosBarracks/VarkaSilenosSupport",
    hierarch: 31377,   // Ashas
    messenger: 31378,  // Naran
    buffer: 31379,     // Udan
    grocer: 31380,     // Diyabu
    warehouse: 31381,  // Hagos
    trader: 31382,     // Shikon
    teleporter: 31383, // Teranu
    currency: 7187,    // Nepenthese Seed
    marks: [7221, 7222, 7223, 7224, 7225],
};

/// Highest alliance mark carried, 0 if none. (Java Varka encodes its levels
/// negative and compares negated — same information, one sign convention.)
fn alliance_level(ctx: &QuestCtx, tribe: &Tribe) -> i32 {
    for (i, mark) in tribe.marks.iter().enumerate() {
        if ctx.item_object_id(*mark).is_some() {
            return (i + 1) as i32;
        }
    }
    0
}

fn first_talk(ctx: &mut QuestCtx, tribe: &Tribe) -> Option<String> {
    let level = alliance_level(ctx, tribe);
    let id = ctx.npc_id;
    let html = if id == tribe.hierarch || id == tribe.messenger || id == tribe.grocer {
        if level > 0 {
            format!("{id}-friend.html")
        } else {
            format!("{id}-no.html")
        }
    } else if id == tribe.buffer {
        match level {
            l if l >= 3 => format!("{id}-04.html"),
            l if l > 0 => format!("{id}-01.html"),
            _ => format!("{id}-03.html"),
        }
    } else if id == tribe.warehouse {
        match level {
            1 => format!("{id}-01.html"),
            l if l > 1 => format!("{id}-02.html"),
            _ => format!("{id}-no.html"),
        }
    } else if id == tribe.trader {
        match level {
            1 | 2 => format!("{id}-01.html"),
            3 | 4 => format!("{id}-02.html"),
            5 => format!("{id}-03.html"),
            _ => format!("{id}-no.html"),
        }
    } else if id == tribe.teleporter {
        match level {
            1..=3 => format!("{id}-01.html"),
            4 => format!("{id}-02.html"),
            5 => format!("{id}-03.html"),
            _ => format!("{id}-no.html"),
        }
    } else {
        return Some(ctx.no_quest_html());
    };
    Some(html)
}

fn on_event(ctx: &mut QuestCtx, tribe: &Tribe, event: &str) -> Option<String> {
    if let Ok(n) = event.parse::<usize>() {
        if (1..=BUFFS.len()).contains(&n) {
            let (skill_id, cost) = BUFFS[n - 1];
            if ctx.take_items(tribe.currency, cost) {
                cast_buff(ctx, skill_id);
                return None;
            }
            return Some(format!("{}-02.html", tribe.buffer));
        }
        return None;
    }
    if event == "Teleport" {
        // The chat window with the destination list; the actual moves are
        // html-side teleport bypasses.
        return match alliance_level(ctx, tribe) {
            4 => Some(format!("{}-04.html", tribe.teleporter)),
            5 => Some(format!("{}-05.html", tribe.teleporter)),
            _ => None,
        };
    }
    None
}

/// `npc.doCast(buff)` + the HP/MP top-up that keeps the buffer from running
/// dry (Java `setCurrentHpMp(max, max)` after every cast).
fn cast_buff(ctx: &mut QuestCtx, skill_id: i32) {
    let npc = ctx.npc;
    let player = ctx.player;
    if let Some(skill) = skill_by_id(ctx.world, skill_id, 1)
        && crate::game_loop::npc::cast::check_use_conditions_pub(ctx.world, npc, &skill)
    {
        crate::game_loop::npc::cast::start_cast(ctx.world, npc, player, &skill);
    }
    if let Some(v) = ctx
        .world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&npc)
    {
        v.cur_hp = v.max_hp as f64;
        v.cur_mp = v.max_mp as f64;
    }
}

macro_rules! tribe_script {
    ($struct_name:ident, $tribe:expr) => {
        pub struct $struct_name;

        impl QuestScript for $struct_name {
            fn id(&self) -> i32 {
                -1
            }
            fn name(&self) -> &'static str {
                $tribe.name
            }
            fn html_dir(&self) -> &'static str {
                $tribe.html_dir
            }
            fn start_npcs(&self) -> &[i32] {
                &[$tribe.buffer, $tribe.teleporter, $tribe.warehouse]
            }
            fn talk_npcs(&self) -> &[i32] {
                &[$tribe.buffer, $tribe.teleporter, $tribe.warehouse]
            }
            fn first_talk_npcs(&self) -> &[i32] {
                &[
                    $tribe.hierarch,
                    $tribe.messenger,
                    $tribe.buffer,
                    $tribe.grocer,
                    $tribe.warehouse,
                    $tribe.trader,
                    $tribe.teleporter,
                ]
            }

            fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
                None
            }

            fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
                first_talk(ctx, &$tribe)
            }

            fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
                on_event(ctx, &$tribe, event)
            }
        }
    };
}

tribe_script!(KetraOrcSupport, KETRA);
tribe_script!(VarkaSilenosSupport, VARKA);
