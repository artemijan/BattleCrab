//! Port of `data/xml/NpcData` scoped to G8's static-world slice: identity,
//! display fields, the base stats `NpcInfo`/targeting need, and the status/ai
//! flags that gate spawning and interaction. Skill lists, drop lists, elemental
//! attributes, mp rewards, and AI parameters wait for the combat/AI milestone
//! (G9) — same "parse what the milestone consumes" pattern as `ItemData` (G5).

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const NPCS_DIR: &str = "data/stats/npcs";

/// One `<item id min max chance>` drop line (Java `DropHolder`, spoil lists
/// not carried — spoiling isn't ported).
#[derive(Debug, Clone, Copy)]
pub struct DropHolder {
    pub item_id: i32,
    pub min: i64,
    pub max: i64,
    /// Percent (0–100, fractional).
    pub chance: f64,
}

/// `<group chance>` around drop lines (Java `DropGroupHolder`).
#[derive(Debug, Clone)]
pub struct DropGroup {
    pub chance: f64,
    pub items: Vec<DropHolder>,
}

/// Java models NPC behaviour as a class per `type` attribute
/// (`model/actor/instance/*`, instantiated by reflection in `Spawn`). The Rust
/// port keeps the type name and derives the two subtree memberships the G8
/// slice actually branches on.
#[derive(Debug, Clone)]
pub struct NpcTemplate {
    pub id: i32,
    /// `displayId` attribute, defaults to `id`; `NpcInfo` sends `+1000000`.
    pub display_id: i32,
    pub level: i32,
    /// The `type` attribute (Java instance-class name: `Folk`, `Monster`, …).
    pub type_name: String,
    pub name: String,
    pub title: String,
    pub server_side_name: bool,
    pub server_side_title: bool,

    // <stats str/int/dex/wit/con/men> — parsed for G9 formulas.
    pub base_str: i32,
    pub base_int: i32,
    pub base_dex: i32,
    pub base_wit: i32,
    pub base_con: i32,
    pub base_men: i32,

    // <vitals> / <attack> / <defence> / <speed> (CreatureTemplate defaults).
    pub base_hp_max: f64,
    pub base_mp_max: f64,
    pub base_hp_reg: f64,
    pub base_mp_reg: f64,
    pub base_p_atk: f64,
    pub base_m_atk: f64,
    pub base_p_def: f64,
    pub base_m_def: f64,
    pub base_p_atk_spd: i32,
    pub base_m_atk_spd: i32,
    pub base_atk_range: i32,
    pub base_walk_spd: f64,
    pub base_run_spd: f64,

    // <collision>
    pub collision_radius: f64,
    pub collision_height: f64,

    // <attack> extras consumed by G9 combat.
    /// `random` attribute → Java `baseRndDam` (`RANDOM_DAMAGE` stat base).
    pub base_rnd_dam: i32,
    /// `critical` attribute → Java `baseCritRate` (pre-DEX/×10 base).
    pub base_crit_rate: f64,

    // <acquire> (consumed in G9, trivially cheap to carry now).
    pub exp: f64,
    pub sp: f64,

    /// `<corpseTime>` (seconds); `None` = `Config.DEFAULT_CORPSE_TIME`.
    pub corpse_time: Option<i32>,

    /// `<dropLists><drop>` — ungrouped death drops (`_dropListDeath`).
    pub drop_list_death: Vec<DropHolder>,
    /// `<dropLists><group chance>` — grouped death drops (`_dropGroups`).
    pub drop_groups: Vec<DropGroup>,

    // <equipment>
    pub rhand: i32,
    pub lhand: i32,

    // <status> flags (NpcTemplate.set defaults).
    pub attackable: bool,
    pub targetable: bool,
    pub talkable: bool,
    pub show_name: bool,
    pub can_move: bool,
    pub random_walk: bool,

    // <ai>
    pub aggro_range: i32,
    pub clan_help_range: i32,
}

impl NpcTemplate {
    /// Membership in Java's `Monster` subtree (`Npc.isMonster()` —
    /// `instanceof Monster`): the auto-attackable mob types.
    pub fn is_monster(&self) -> bool {
        matches!(
            self.type_name.as_str(),
            "Monster" | "Chest" | "ControllableMob" | "EventMonster" | "FeedableBeast" | "TamedBeast" | "GrandBoss" | "RaidBoss" | "FestivalMonster"
        )
    }

    /// Membership in Java's `Attackable` subtree (`Creature.isAttackable()`
    /// override) — what `NpcInfo`'s ATTACKABLE byte and the HP status-bar on
    /// target derive from.
    pub fn is_attackable_class(&self) -> bool {
        self.is_monster()
            || matches!(
                self.type_name.as_str(),
                "Guard" | "Defender" | "FortCommander" | "Doppelganger" | "FriendlyMob" | "FriendlyNpc"
            )
    }

    /// `isAutoAttackable(Player)` narrowed to the no-PvP-state world: only the
    /// `Monster` subtree attacks/is attacked without force-use (Guards only
    /// auto-attack karma players, which don't exist yet).
    pub fn is_auto_attackable(&self) -> bool {
        self.is_monster()
    }
}

pub struct NpcData {
    by_id: HashMap<i32, NpcTemplate>,
}

impl NpcData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_id = HashMap::new();
        let dir = format!("{file_path}{NPCS_DIR}");
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut paths: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("xml"))
                .collect();
            paths.sort();
            for path in paths {
                parse_file(&path, &mut by_id);
            }
        }
        info!("NpcData: Loaded {} NPCs.", by_id.len());
        Self { by_id }
    }

    pub fn get(&self, npc_id: i32) -> Option<&NpcTemplate> {
        self.by_id.get(&npc_id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { by_id: HashMap::new() }
    }

    /// Synthetic catalog for unit tests (same hook as `ItemData::from_templates`).
    #[doc(hidden)]
    pub fn from_templates(templates: Vec<NpcTemplate>) -> Self {
        Self { by_id: templates.into_iter().map(|t| (t.id, t)).collect() }
    }

    /// Register one synthetic template (same hook as `SkillData::insert_for_test`).
    #[doc(hidden)]
    pub fn insert_for_test(&mut self, t: NpcTemplate) {
        self.by_id.insert(t.id, t);
    }
}

/// A blank template with Java's `CreatureTemplate`/`NpcTemplate` defaults,
/// filled in by the parser (and handy for tests).
pub fn default_template(id: i32) -> NpcTemplate {
    NpcTemplate {
        id,
        display_id: id,
        level: 85,
        type_name: "Folk".to_string(),
        name: String::new(),
        title: String::new(),
        server_side_name: false,
        server_side_title: false,
        base_str: 40,
        base_int: 21,
        base_dex: 30,
        base_wit: 20,
        base_con: 43,
        base_men: 25,
        base_hp_max: 0.0,
        base_mp_max: 0.0,
        base_hp_reg: 0.0,
        base_mp_reg: 0.0,
        base_p_atk: 0.0,
        base_m_atk: 0.0,
        base_p_def: 0.0,
        base_m_def: 0.0,
        base_p_atk_spd: 300,
        base_m_atk_spd: 333,
        base_atk_range: 40,
        base_walk_spd: 50.0,
        base_run_spd: 120.0,
        collision_radius: 0.0,
        collision_height: 0.0,
        base_rnd_dam: 0,
        base_crit_rate: 4.0,
        exp: 0.0,
        sp: 0.0,
        corpse_time: None,
        drop_list_death: Vec::new(),
        drop_groups: Vec::new(),
        rhand: 0,
        lhand: 0,
        attackable: true,
        targetable: true,
        talkable: true,
        show_name: true,
        can_move: true,
        random_walk: false,
        aggro_range: 0,
        clan_help_range: 0,
    }
}

fn parse_file(path: &std::path::Path, out: &mut HashMap<i32, NpcTemplate>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);

    let mut cur: Option<NpcTemplate> = None;
    // `<attack>`/`<defence>` exist both under `<stats>` (what we want) and
    // under `<stats><attribute>` (elemental values, skipped) — track the
    // `<attribute>` scope to tell them apart.
    let mut in_attribute = false;
    // `<dropLists>` scope: which list `<item>` lines belong to.
    #[derive(PartialEq)]
    enum DropScope {
        None,
        Death,
        Spoil,
    }
    let mut drop_scope = DropScope::None;
    let mut cur_group: Option<DropGroup> = None;
    // `<corpseTime>` carries its value as element text.
    let mut in_corpse_time = false;

    while let Ok(event) = reader.read_event() {
        let (e, self_closing) = match event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::Text(text) => {
                if in_corpse_time {
                    if let Some(t) = cur.as_mut() {
                        if let Ok(v) = String::from_utf8_lossy(&text).trim().parse() {
                            t.corpse_time = Some(v);
                        }
                    }
                }
                continue;
            }
            Event::End(e) => {
                match e.name().as_ref().to_ascii_lowercase().as_slice() {
                    b"npc" => {
                        if let Some(t) = cur.take() {
                            finish_template(t, out);
                        }
                    }
                    b"attribute" => in_attribute = false,
                    b"corpsetime" => in_corpse_time = false,
                    b"drop" | b"spoil" => drop_scope = DropScope::None,
                    b"group" => {
                        if let (Some(t), Some(g)) = (cur.as_mut(), cur_group.take()) {
                            t.drop_groups.push(g);
                        }
                    }
                    _ => {}
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let name = e.name().as_ref().to_ascii_lowercase();
        match name.as_slice() {
                    b"npc" => {
                        let Some(id) = attr_i32(&e, b"id") else { continue };
                        let mut t = default_template(id);
                        if let Some(v) = attr_i32(&e, b"displayId") {
                            t.display_id = v;
                        }
                        if let Some(v) = attr_i32(&e, b"level") {
                            t.level = v;
                        }
                        if let Some(v) = attr_str(&e, b"type") {
                            t.type_name = v;
                        }
                        t.name = attr_str(&e, b"name").unwrap_or_default();
                        t.title = attr_str(&e, b"title").unwrap_or_default();
                        // NpcTemplate.set: randomWalk defaults to `!type.equals("Guard")`.
                        t.random_walk = t.type_name != "Guard";
                        t.server_side_name = attr_bool(&e, b"usingServerSideName").unwrap_or(false);
                        t.server_side_title = attr_bool(&e, b"usingServerSideTitle").unwrap_or(false);
                        cur = Some(t);
                    }
                    b"attribute" => in_attribute = true,
                    b"stats" => {
                        if let Some(t) = cur.as_mut() {
                            set_i32(&e, b"str", &mut t.base_str);
                            set_i32(&e, b"int", &mut t.base_int);
                            set_i32(&e, b"dex", &mut t.base_dex);
                            set_i32(&e, b"wit", &mut t.base_wit);
                            set_i32(&e, b"con", &mut t.base_con);
                            set_i32(&e, b"men", &mut t.base_men);
                        }
                    }
                    b"vitals" => {
                        if let Some(t) = cur.as_mut() {
                            set_f64(&e, b"hp", &mut t.base_hp_max);
                            set_f64(&e, b"mp", &mut t.base_mp_max);
                            set_f64(&e, b"hpRegen", &mut t.base_hp_reg);
                            set_f64(&e, b"mpRegen", &mut t.base_mp_reg);
                        }
                    }
                    b"attack" if !in_attribute => {
                        if let Some(t) = cur.as_mut() {
                            set_f64(&e, b"physical", &mut t.base_p_atk);
                            set_f64(&e, b"magical", &mut t.base_m_atk);
                            if let Some(v) = attr_f64(&e, b"attackSpeed") {
                                t.base_p_atk_spd = v as i32;
                            }
                            set_i32(&e, b"range", &mut t.base_atk_range);
                            set_i32(&e, b"random", &mut t.base_rnd_dam);
                            set_f64(&e, b"critical", &mut t.base_crit_rate);
                        }
                    }
                    b"defence" if !in_attribute => {
                        if let Some(t) = cur.as_mut() {
                            set_f64(&e, b"physical", &mut t.base_p_def);
                            set_f64(&e, b"magical", &mut t.base_m_def);
                        }
                    }
                    b"walk" => {
                        if let Some(t) = cur.as_mut() {
                            if let Some(v) = attr_f64(&e, b"ground") {
                                // NpcData: `groundWalk <= 0 → 0.1`.
                                t.base_walk_spd = if v <= 0.0 { 0.1 } else { v };
                            }
                        }
                    }
                    b"run" => {
                        if let Some(t) = cur.as_mut() {
                            if let Some(v) = attr_f64(&e, b"ground") {
                                t.base_run_spd = if v <= 0.0 { 0.1 } else { v };
                            }
                        }
                    }
                    b"acquire" => {
                        if let Some(t) = cur.as_mut() {
                            set_f64(&e, b"exp", &mut t.exp);
                            set_f64(&e, b"sp", &mut t.sp);
                        }
                    }
                    b"equipment" => {
                        if let Some(t) = cur.as_mut() {
                            set_i32(&e, b"rhand", &mut t.rhand);
                            set_i32(&e, b"lhand", &mut t.lhand);
                        }
                    }
                    b"status" => {
                        if let Some(t) = cur.as_mut() {
                            t.attackable = attr_bool(&e, b"attackable").unwrap_or(true);
                            t.targetable = attr_bool(&e, b"targetable").unwrap_or(true);
                            t.talkable = attr_bool(&e, b"talkable").unwrap_or(true);
                            t.show_name = attr_bool(&e, b"showName").unwrap_or(true);
                            t.can_move = attr_bool(&e, b"canMove").unwrap_or(true);
                            if let Some(v) = attr_bool(&e, b"randomWalk") {
                                t.random_walk = v;
                            }
                        }
                    }
                    b"ai" => {
                        if let Some(t) = cur.as_mut() {
                            set_i32(&e, b"aggroRange", &mut t.aggro_range);
                            set_i32(&e, b"clanHelpRange", &mut t.clan_help_range);
                        }
                    }
                    b"corpsetime" => in_corpse_time = !self_closing,
                    b"drop" => drop_scope = DropScope::Death,
                    b"spoil" => drop_scope = DropScope::Spoil,
                    b"group" => {
                        cur_group = Some(DropGroup { chance: attr_f64(&e, b"chance").unwrap_or(0.0), items: Vec::new() });
                    }
                    b"item" if drop_scope != DropScope::None || cur_group.is_some() => {
                        let holder = DropHolder {
                            item_id: attr_i32(&e, b"id").unwrap_or(0),
                            min: attr_i32(&e, b"min").unwrap_or(1) as i64,
                            max: attr_i32(&e, b"max").unwrap_or(1) as i64,
                            chance: attr_f64(&e, b"chance").unwrap_or(0.0),
                        };
                        if let Some(g) = cur_group.as_mut() {
                            g.items.push(holder);
                        } else if drop_scope == DropScope::Death {
                            if let Some(t) = cur.as_mut() {
                                t.drop_list_death.push(holder);
                            }
                        }
                        // Spoil lists are dropped — spoiling isn't ported.
                    }
                    b"radius" => {
                        if let Some(t) = cur.as_mut() {
                            set_f64(&e, b"normal", &mut t.collision_radius);
                        }
                    }
                    b"height" => {
                        if let Some(t) = cur.as_mut() {
                            set_f64(&e, b"normal", &mut t.collision_height);
                        }
                    }
                    _ => {}
                }
        // Self-closing `<npc …/>` (no children) — no End event follows.
        if self_closing && name.as_slice() == b"npc" {
            if let Some(t) = cur.take() {
                finish_template(t, out);
            }
        }
    }
}

fn finish_template(mut t: NpcTemplate, out: &mut HashMap<i32, NpcTemplate>) {
    // NpcTemplate.set: `_canMove = baseWalkSpd <= 0.1 || canMove` — the 0.1
    // walk-speed sentinel (see the `<walk>` branch) means "pinned in place",
    // but Java ORs it, so it *enables* movement. Kept faithfully.
    if t.base_walk_spd <= 0.1 {
        t.can_move = true;
    }
    out.insert(t.id, t);
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn attr_i32(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

fn attr_f64(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<f64> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

fn attr_bool(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<bool> {
    attr_str(e, key).map(|v| v == "true")
}

fn set_i32(e: &quick_xml::events::BytesStart, key: &[u8], dst: &mut i32) {
    if let Some(v) = attr_i32(e, key) {
        *dst = v;
    }
}

fn set_f64(e: &quick_xml::events::BytesStart, key: &[u8], dst: &mut f64) {
    if let Some(v) = attr_f64(e, key) {
        *dst = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_dist_files() {
        let data = NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // Java startup: "NpcData: Loaded ~13k NPCs" — pin a floor, not the
        // exact count, so datapack tweaks don't churn the test.
        assert!(data.len() > 10_000, "expected >10k NPC templates, got {}", data.len());

        // Thomas D. Turkey (id 100) — the first template in 00100-00199.xml,
        // hand-checked against the XML.
        let t = data.get(100).expect("npc 100");
        assert_eq!(t.name, "Thomas D. Turkey");
        assert_eq!(t.level, 80);
        assert_eq!(t.type_name, "Folk");
        assert_eq!(t.base_hp_max, 3290.0);
        assert_eq!(t.base_p_atk_spd, 253);
        assert_eq!(t.base_run_spd, 160.0);
        assert_eq!(t.collision_radius, 25.0);
        assert_eq!(t.collision_height, 35.0);
        assert_eq!(t.aggro_range, 300);
        assert!(!t.is_monster());

        // Elemental <attribute><defence fire=…/> must not clobber base_p_def.
        assert_eq!(t.base_p_def, 341.375);
        assert_eq!(t.base_m_def, 249.80341);

        // A real monster: Gremlin (id 20001).
        let g = data.get(20001).expect("npc 20001");
        assert!(g.is_monster());
        assert!(g.is_attackable_class());
        // <attack random critical> feed the G9 combat formulas.
        assert_eq!(g.base_rnd_dam, 30);
        assert_eq!(g.base_crit_rate, 4.75);

        // Goblin (20003): drop list + aggro range, hand-checked from the XML.
        let goblin = data.get(20003).expect("npc 20003");
        assert_eq!(goblin.aggro_range, 450);
        assert_eq!(goblin.drop_list_death.len(), 9);
        let adena = goblin.drop_list_death.iter().find(|d| d.item_id == 57).expect("adena line");
        assert_eq!((adena.min, adena.max), (13, 30));
        assert_eq!(adena.chance, 70.0);
        // Spoil lines are not carried.
        assert!(goblin.drop_groups.is_empty());

        // <corpseTime> element text (npc 103, Holiday Santa); absent = None.
        assert_eq!(data.get(103).unwrap().corpse_time, Some(3));
        assert_eq!(g.corpse_time, None);
    }

    /// The one dist file with `<group chance>` drops (Primeval Isle mobs).
    #[test]
    fn grouped_drops_parse() {
        let data = NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        let t = data.get(22119).or_else(|| data.get(22100)).expect("a 221xx monster");
        // Every monster in that file carries grouped drops.
        assert!(!t.drop_groups.is_empty(), "npc {} should have drop groups", t.id);
        let group = &t.drop_groups[0];
        assert!(group.chance > 0.0);
        assert!(!group.items.is_empty());
    }

    #[test]
    fn status_flags_parse() {
        let data = NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // Npc 101: <status attackable="false" canMove="false"/>.
        let t = data.get(101).expect("npc 101");
        assert!(!t.attackable);
        assert!(!t.can_move);
    }
}
