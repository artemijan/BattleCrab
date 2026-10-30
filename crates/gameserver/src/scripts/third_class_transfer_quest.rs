//! Day of Destiny (10341–10346) — port of
//! `dist/game/data/scripts/quests/ThirdClassTransferQuest.java` and its six
//! race subclasses (`Q10341_DayOfDestinyHumansFate` …
//! `Q10346_DayOfDestinyKamaelsFate`), the Salvation third class transfer.
//!
//! The race's master starts the quest from level 76 and remembers the class
//! it started with. The Quartermaster sends the player to collect the race's
//! tag from the four fallen Vanguards (cond 2 → 3), takes them back (4), and
//! the Vanguard Member sends the player into the Evil Incubator
//! ([`super::evil_incubator`]), which drives cond 5–13. Back at the master,
//! `classTransfer` moves the player to the class's single next class.
//!
//! Java's six subclasses differ only in the master, the race and the quest
//! id, so one [`DayOfDestiny`] carries those and the base class's logic.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::class_names;

// NPCs
const QUARTERMASTER: i32 = 33407;
const VANGUARD_MEMBER: i32 = 33165;
const VANGUARDS: [i32; 4] = [33166, 33167, 33168, 33169];

// Items
const SOULSHOTS: i32 = 1467;
const SPIRITSHOTS: i32 = 3952;
const BLESSED_SCROLL_OF_RESURRECTION: i32 = 33518;
const PAULINAS_EQUIPMENT_SET: i32 = 46852;

/// The race tags (`RACE_TAGS`) and the Cry of Destiny pendants, all
/// registered as quest items.
const QUEST_ITEMS: [i32; 40] = [
    17748, 17749, 17750, 17751, 17752, 17753, // race tags
    17484, 17485, 17486, 17487, 17488, 17489, 17490, 17491, 17492, 17493, 17494, 17495, 17496,
    17497, 17498, 17499, 17500, 17501, 17502, 17503, 17504, 17505, 17506, 17507, 17508, 17509,
    17510, 17511, 17512, 17513, 17514, 17515, 17516, 17517,
];

// Skills
const SHOW_SKILL: (i32, i32) = (5103, 1);

// Misc
const QUESTION_MARK_ID: i32 = 101;
const MIN_LEVEL: i32 = 76;
/// `SystemMessageId.CONGRATULATIONS_YOU_VE_COMPLETED_YOUR_THIRD_CLASS_TRANSFER_QUEST`.
const SM_THIRD_CLASS_TRANSFER_COMPLETE: i16 = 1606;

// `Race` ordinals.
const HUMAN: i32 = 0;
const ELF: i32 = 1;
const DARK_ELF: i32 = 2;
const ORC: i32 = 3;
const DWARF: i32 = 4;
const KAMAEL: i32 = 5;

/// One race's Day of Destiny.
pub struct DayOfDestiny {
    id: i32,
    name: &'static str,
    html_dir: &'static str,
    master: [i32; 1],
    /// The master (`addTalkId` in each subclass), then the base class's
    /// Quartermaster, Vanguard Member and Vanguards.
    talk: [i32; 7],
    race: i32,
}

impl DayOfDestiny {
    const fn new(
        id: i32,
        name: &'static str,
        html_dir: &'static str,
        master: i32,
        race: i32,
    ) -> Self {
        Self {
            id,
            name,
            html_dir,
            master: [master],
            talk: [
                master,
                QUARTERMASTER,
                VANGUARD_MEMBER,
                VANGUARDS[0],
                VANGUARDS[1],
                VANGUARDS[2],
                VANGUARDS[3],
            ],
            race,
        }
    }
}

pub const HUMANS_FATE: DayOfDestiny = DayOfDestiny::new(
    10341,
    "Q10341_DayOfDestinyHumansFate",
    "quests/Q10341_DayOfDestinyHumansFate",
    30857, // Orven
    HUMAN,
);
pub const ELVEN_FATE: DayOfDestiny = DayOfDestiny::new(
    10342,
    "Q10342_DayOfDestinyElvenFate",
    "quests/Q10342_DayOfDestinyElvenFate",
    30856, // Winonin
    ELF,
);
pub const DARK_ELFS_FATE: DayOfDestiny = DayOfDestiny::new(
    10343,
    "Q10343_DayOfDestinyDarkElfsFate",
    "quests/Q10343_DayOfDestinyDarkElfsFate",
    30862, // Oltran
    DARK_ELF,
);
pub const ORCS_FATE: DayOfDestiny = DayOfDestiny::new(
    10344,
    "Q10344_DayOfDestinyOrcsFate",
    "quests/Q10344_DayOfDestinyOrcsFate",
    30865, // Ladanza
    ORC,
);
pub const DWARFS_FATE: DayOfDestiny = DayOfDestiny::new(
    10345,
    "Q10345_DayOfDestinyDwarfsFate",
    "quests/Q10345_DayOfDestinyDwarfsFate",
    30847, // Ferris
    DWARF,
);
pub const KAMAELS_FATE: DayOfDestiny = DayOfDestiny::new(
    10346,
    "Q10346_DayOfDestinyKamaelsFate",
    "quests/Q10346_DayOfDestinyKamaelsFate",
    32221, // Brome
    KAMAEL,
);

/// The quest a race's Evil Incubator run belongs to (`EvilIncubator.RACE_QUESTS`).
pub fn quest_name_for_race(race: i32) -> Option<&'static str> {
    [
        &HUMANS_FATE,
        &ELVEN_FATE,
        &DARK_ELFS_FATE,
        &ORCS_FATE,
        &DWARFS_FATE,
        &KAMAELS_FATE,
    ]
    .into_iter()
    .find(|q| q.race == race)
    .map(|q| q.name)
}

/// The race tag `RACE_TAGS.get(race)`.
fn race_tag(race: i32) -> Option<i32> {
    (HUMAN..=KAMAEL).contains(&race).then(|| 17748 + race)
}

impl DayOfDestiny {
    fn master(&self) -> i32 {
        self.master[0]
    }

    /// `player.getPlayerClass().getNextClasses().stream().findFirst()`. Every
    /// class of `THIRD_CLASS_GROUP` has exactly one.
    fn next_class(ctx: &QuestCtx) -> Option<i32> {
        ctx.world
            .data
            .skill_trees
            .next_classes(ctx.player_class_id())
            .first()
            .copied()
    }

    /// The guard `nextClassInfo` and `classTransfer` share: the player
    /// changed class since the start and is still of the level.
    fn changed_class_page(&self, ctx: &QuestCtx) -> Option<String> {
        (ctx.get_int("STARTED_CLASS") != ctx.player_class_id() && ctx.player_level() >= MIN_LEVEL)
            .then(|| format!("{}-10.html", ctx.npc_id))
    }

    fn collect_tag(&self, ctx: &mut QuestCtx) -> Option<String> {
        if !ctx.is_cond(2) {
            return None;
        }
        let tag = race_tag(ctx.player_race())?;
        // Java's `1 << (VANGUARDS[0] - npc.getId())`: a shift by 0, -1, -2,
        // -3, which Java masks to 0, 31, 30, 29.
        let bit = 1i32.wrapping_shl((VANGUARDS[0] - ctx.npc_id) as u32);
        let vanguard = ctx.get_int("vanguard");
        if vanguard & bit == bit {
            return Some("vanguard-03.html".to_string());
        }
        ctx.give_items(tag, 1);
        ctx.set_var("vanguard", (vanguard | bit).to_string());
        if ctx.quest_items_count(tag) == 4 {
            ctx.set_cond(3, true);
            Some("vanguard-04.html".to_string())
        } else {
            Some("vanguard-02.html".to_string())
        }
    }

    fn class_transfer(&self, ctx: &mut QuestCtx) -> Option<String> {
        if let Some(html) = self.changed_class_page(ctx) {
            return Some(html);
        }
        let new_class = Self::next_class(ctx)?;
        let (npc, player) = (ctx.npc, ctx.player);
        ctx.npc_cast(npc, player, SHOW_SKILL.0, SHOW_SKILL.1);
        ctx.send_sm(SM_THIRD_CLASS_TRANSFER_COMPLETE);
        ctx.broadcast_social_action(3);
        ctx.set_class_id(new_class);
        ctx.give_items(SOULSHOTS, 8000);
        ctx.give_items(SPIRITSHOTS, 8000);
        ctx.give_items(BLESSED_SCROLL_OF_RESURRECTION, 3);
        ctx.give_items(PAULINAS_EQUIPMENT_SET, 1);
        ctx.add_exp_and_sp(42_000_000, 0);
        ctx.exit_quest(true, true);
        Some(format!("{}-09.html", ctx.npc_id))
    }

    /// `ThirdClassTransferQuest.onEvent`.
    fn base_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            "33407-02.html" => {
                if !ctx.is_cond(1) {
                    return None;
                }
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "33407-05.html" => {
                if !ctx.is_cond(3) {
                    return None;
                }
                ctx.set_cond(4, true);
                ctx.unset("vanguard");
                if let Some(tag) = race_tag(ctx.player_race()) {
                    ctx.take_items(tag, -1);
                }
                Some(event.to_string())
            }
            "33165-02.html" => {
                if !ctx.is_cond(4) {
                    return None;
                }
                ctx.set_cond(5, true);
                Some(event.to_string())
            }
            "collectTag" => self.collect_tag(ctx),
            "nextClassInfo" => {
                if let Some(html) = self.changed_class_page(ctx) {
                    return Some(html);
                }
                let next = Self::next_class(ctx)?;
                let name = class_names::class_name(next)?.to_lowercase();
                Some(format!("class_preview_{name}.html"))
            }
            "classTransfer" => self.class_transfer(ctx),
            _ => None,
        }
    }

    /// `ThirdClassTransferQuest.onTalk`.
    fn base_talk(&self, ctx: &QuestCtx) -> Option<String> {
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            QUARTERMASTER => match cond {
                1 => "33407-01.html",
                2 => "33407-03.html",
                3 => "33407-04.html",
                4..=12 => "33407-05.html",
                _ => return Some(ctx.no_quest_html()),
            },
            VANGUARD_MEMBER if (4..=12).contains(&cond) => "33165-01.html",
            npc if cond == 2 && VANGUARDS.contains(&npc) => "vanguard-01.html",
            _ => return Some(ctx.no_quest_html()),
        };
        Some(html.to_string())
    }

    fn may_see_mark(&self, ctx: &QuestCtx) -> bool {
        ctx.player_race() == self.race && ctx.is_in_category("THIRD_CLASS_GROUP")
    }
}

impl QuestScript for DayOfDestiny {
    fn id(&self) -> i32 {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn html_dir(&self) -> &'static str {
        self.html_dir
    }
    fn start_npcs(&self) -> &[i32] {
        &self.master
    }
    fn talk_npcs(&self) -> &[i32] {
        &self.talk
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }
    fn handles_global_events(&self) -> bool {
        true
    }

    /// `addCondMinLevel(76, "-11.html")`, `addCondRace(race, "-11.html")`
    /// and `addCondInCategory(THIRD_CLASS_GROUP, "-12.html")`, in order.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        let master = self.master();
        if ctx.player_level() < MIN_LEVEL || ctx.player_race() != self.race {
            return Some(format!("{master}-11.html"));
        }
        if !ctx.is_in_category("THIRD_CLASS_GROUP") {
            return Some(format!("{master}-12.html"));
        }
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        let master = self.master();
        if event == format!("{master}-02.htm")
            || event == format!("{master}-03.htm")
            || event == format!("{master}-04.htm")
            || event == format!("{master}-08.html")
        {
            return Some(event.to_string());
        }
        if event == format!("{master}-05.htm") {
            ctx.start_quest();
            let class_id = ctx.player_class_id();
            ctx.set_var("STARTED_CLASS", class_id.to_string());
            return Some(event.to_string());
        }
        self.base_event(ctx, event)
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let master = self.master();
        if ctx.npc_id == master {
            if ctx.is_created() {
                return Some(format!("{master}-01.htm"));
            }
            if ctx.is_started() {
                if ctx.is_cond(1) {
                    return Some(format!("{master}-06.html"));
                }
                if ctx.is_cond(13) {
                    return Some(format!("{master}-07.html"));
                }
            }
        }
        self.base_talk(ctx)
    }

    /// `ON_PLAYER_PRESS_TUTORIAL_MARK`: mark 101, shared by the six quests,
    /// opens the invitation of the player's race.
    fn on_tutorial_mark(&self, ctx: &mut QuestCtx, mark_id: i32) {
        if mark_id == QUESTION_MARK_ID && ctx.player_race() == self.race {
            ctx.tutorial_show_html_file("popupInvite.html");
        }
    }

    /// `ON_PLAYER_LEVEL_CHANGED`: reaching exactly level 76 shows mark 101.
    fn on_level_changed(&self, ctx: &mut QuestCtx, old_level: i32, new_level: i32) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if old_level < new_level && new_level == MIN_LEVEL && self.may_see_mark(ctx) {
            ctx.tutorial_show_question_mark(QUESTION_MARK_ID, 1);
        }
    }

    /// `ON_PLAYER_LOGIN`: from level 76 on, mark 101 until the quest starts.
    fn on_login(&self, ctx: &mut QuestCtx) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if ctx.player_level() >= MIN_LEVEL
            && self.may_see_mark(ctx)
            && (!ctx.has_qs() || ctx.is_created())
        {
            ctx.tutorial_show_question_mark(QUESTION_MARK_ID, 1);
        }
    }
}
