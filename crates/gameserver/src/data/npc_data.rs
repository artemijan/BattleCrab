//! Port of `data/xml/NpcData` scoped to G8's static-world slice: identity,
//! display fields, the base stats `NpcInfo`/targeting need, and the status/ai
//! flags that gate spawning and interaction. Skill lists, drop lists, elemental
//! attributes, mp rewards, and AI parameters wait for the combat/AI milestone
//! (G9) — same "parse what the milestone consumes" pattern as `ItemData` (G5).

use std::collections::HashMap;
use std::sync::OnceLock;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const NPCS_DIR: &str = "data/stats/npcs";

/// One `<item id min max chance>` drop line (Java `DropHolder`), shared by the
/// death (`<drop>`) and spoil (`<spoil>`) lists.
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
/// Java `model/holders/MinionHolder` — one `<npc>` row of a `<minions>` block.
#[derive(Debug, Clone, Copy)]
pub struct MinionHolder {
    pub npc_id: i32,
    /// How many of this minion the leader keeps alive at once.
    pub count: i32,
}

/// Java `enums/AIType` — the `<ai type="…">` attribute. This dist uses
/// `BALANCED` (3163), `MAGE` (402), `ARCHER` (220), `CORPSE` (43) and
/// `HEALER` (23); everything else omits the attribute and defaults to
/// `FIGHTER`. `AttackableAI` only branches on `MAGE` and `ARCHER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiType {
    #[default]
    Fighter,
    Archer,
    Balanced,
    Mage,
    Healer,
    Corpse,
}

impl AiType {
    fn parse(s: &str) -> Self {
        match s {
            "ARCHER" => Self::Archer,
            "BALANCED" => Self::Balanced,
            "MAGE" => Self::Mage,
            "HEALER" => Self::Healer,
            "CORPSE" => Self::Corpse,
            _ => Self::Fighter,
        }
    }
}

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
    /// `<race>` → Java `NpcTemplate.getRace()`, the full `Race` enum ordinal
    /// (playable races *and* the creature-category values — `UNDEAD`,
    /// `BEAST`, … — G19's `AttackTrait`/`*_WEAKNESS` reads those). `None` for
    /// the templates that declare no race — Java leaves the field null
    /// there, and a null never equals anything, playable or not.
    pub race: Option<i32>,

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
    /// `<dropLists><spoil>` — the Sweeper loot list (`_dropListSpoil`). Rolled
    /// (`DropType.SPOIL`) into `Attackable._sweepItems` only when the mob dies
    /// spoiled; also previewed by the shift-click "Show Spoil" NPC view.
    pub drop_list_spoil: Vec<DropHolder>,

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
    /// Java `isRandomAnimationEnabled` (`randomAnimation`, default true) — the
    /// NPC plays idle social animations while standing around.
    pub random_animation: bool,

    // <ai>
    /// Java `isAggressive` (default false). `aggroRange` alone is *not*
    /// aggression — most passive mobs (even Rabbit 20002) carry
    /// `aggroRange="450"`; it's only the scan radius when this flag is set.
    pub is_aggressive: bool,
    pub aggro_range: i32,
    pub clan_help_range: i32,
    /// `<ai type="…">` (Java `NpcTemplate._aiType`, default `FIGHTER`). Only
    /// `MAGE` changes behaviour in `AttackableAI`: a mage casts on every think
    /// without the `hasSkillChance()` roll and without having to stand still.
    /// `ARCHER` additionally keeps its distance (the kite move, not ported).
    pub ai_type: AiType,
    /// `minSkillChance`/`maxSkillChance` (Java defaults 7/15). `hasSkillChance()`
    /// is `Rnd.get(100) < Rnd.get(min, max)`, i.e. roughly a 1-in-9 chance per
    /// think for a non-mage. Neither attribute appears anywhere in this dist, so
    /// every NPC uses the defaults — parsed anyway to stay data-driven.
    pub min_skill_chance: i32,
    pub max_skill_chance: i32,
    /// `<ai><clanList><clan>…` (Java `NpcTemplate._clans`) — the faction names
    /// this NPC belongs to. Two NPCs share a faction if their sets intersect,
    /// or if either side declares `ALL`. 4569 clan entries on this dist.
    pub clans: Vec<String>,
    /// `<parameters><minions><npc id count/>` (Java `NpcTemplate.getParameters()
    /// .getMinionList("Privates")`) — the escort this NPC spawns with. 467
    /// leaders on this dist declare 962 minion entries.
    pub minions: Vec<MinionHolder>,
    /// `<ai><clanList><ignoreNpcId>…` (Java `_ignoreClanNpcIds`) — faction-mates
    /// this NPC refuses to answer help calls from, even sharing a clan.
    pub ignore_clan_npc_ids: Vec<i32>,

    /// `<skillList><skill id level/>` — the template skills Java copies onto the
    /// creature in the `Creature` constructor (`for (Skill s : template.getSkills())
    /// addSkill(s)`). The *passive* ones (operateType `P`: 4408 HP Increase,
    /// 4410 P.Atk, 4412 P.Def, …) carry the `MaxHp`/`PAtk`/… stat effects that a
    /// retail mob's HP/atk/def are built from — without them an NPC shows only
    /// its raw `<vitals>`/`<attack>` base. NOT the `<parameters><skill name=..>`
    /// AI holders (those are parameters, not `getSkills()`). Stored as
    /// `(skillId, level)`; effects are resolved against `SkillData` at spawn.
    pub skill_list: Vec<(i32, i32)>,
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

    /// Java `me instanceof Guard` — the town guards that hunt PKs. 186 NPCs on
    /// this dist. Distinct from the siege `Defender` guards, which have their
    /// own aggro path.
    pub fn is_guard(&self) -> bool {
        self.type_name == "Guard"
    }

    /// Java `NpcTemplate.isClan(Set<Integer> clans)` — do these two factions
    /// overlap? `ALL` on *either* side matches everything, which is how the
    /// 238 `ALL` NPCs pull their whole neighbourhood into a fight.
    pub fn shares_clan_with(&self, other: &NpcTemplate) -> bool {
        if self.clans.is_empty() || other.clans.is_empty() {
            return false;
        }
        if self.clans.iter().any(|c| c == "ALL") || other.clans.iter().any(|c| c == "ALL") {
            return true;
        }
        self.clans.iter().any(|c| other.clans.contains(c))
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

    /// `Npc.isRaid()` — `instanceof RaidBoss` (which `GrandBoss` extends).
    /// Used by the `NpcViewMod` drop-list preview to pick the raid rate.
    pub fn is_raid(&self) -> bool {
        matches!(self.type_name.as_str(), "RaidBoss" | "GrandBoss")
    }
}

/// One flattened drop entry for the community-board drop search (Java
/// `DropSearchBoard.CBDropHolder`): a single item's appearance in one NPC's
/// drop or spoil list, with the group chance already folded in.
#[derive(Debug, Clone)]
pub struct CbDrop {
    pub item_id: i32,
    pub npc_id: i32,
    pub npc_level: i32,
    pub min: i64,
    pub max: i64,
    /// Percent (0–100), group chance already folded (Java `chance / 100`).
    pub chance: f64,
    pub is_spoil: bool,
    pub is_raid: bool,
}

pub struct NpcData {
    by_id: HashMap<i32, NpcTemplate>,
    /// Lazily-built inverted index `item_id → [CbDrop]` for the community-board
    /// drop search (Java builds it eagerly in `DropSearchBoard`'s constructor;
    /// this port defers the ~35k-template scan until the first search click).
    drop_index: OnceLock<HashMap<i32, Vec<CbDrop>>>,
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
        Self { by_id, drop_index: OnceLock::new() }
    }

    pub fn get(&self, npc_id: i32) -> Option<&NpcTemplate> {
        self.by_id.get(&npc_id)
    }

    /// Java `NpcData.getTemplateByName` — first template whose name matches
    /// `name` case-insensitively (linear scan, as in Java). Used by the admin
    /// spawn commands, whose "Id/Name" input accepts a name in place of an id.
    pub fn get_by_name(&self, name: &str) -> Option<&NpcTemplate> {
        self.by_id.values().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Java `NpcData.getAllMonstersOfLevel(level)` — templates of exactly
    /// `level` whose type is `Monster` (Java `isType`, case-insensitive). Sorted
    /// by id so the admin spawn-by-level menu's `Next` pagination is stable
    /// across calls (Java relies on unspecified HashMap order).
    pub fn monsters_of_level(&self, level: i32) -> Vec<&NpcTemplate> {
        let mut v: Vec<&NpcTemplate> =
            self.by_id.values().filter(|t| t.level == level && t.type_name.eq_ignore_ascii_case("Monster")).collect();
        v.sort_by_key(|t| t.id);
        v
    }

    /// Java `NpcData.getAllNpcStartingWith(text)` — `Folk`-type templates whose
    /// name has `text` as a (case-sensitive) prefix. Sorted by id for stable
    /// pagination, as with [`monsters_of_level`].
    pub fn folk_starting_with(&self, text: &str) -> Vec<&NpcTemplate> {
        let mut v: Vec<&NpcTemplate> =
            self.by_id.values().filter(|t| t.type_name.eq_ignore_ascii_case("Folk") && t.name.starts_with(text)).collect();
        v.sort_by_key(|t| t.id);
        v
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { by_id: HashMap::new(), drop_index: OnceLock::new() }
    }

    /// Synthetic catalog for unit tests (same hook as `ItemData::from_templates`).
    #[doc(hidden)]
    pub fn from_templates(templates: Vec<NpcTemplate>) -> Self {
        Self { by_id: templates.into_iter().map(|t| (t.id, t)).collect(), drop_index: OnceLock::new() }
    }

    /// Every loaded template (Java `NpcData.getTemplates`), unordered.
    pub fn all(&self) -> impl Iterator<Item = &NpcTemplate> {
        self.by_id.values()
    }

    /// The community-board drop index (Java `DropSearchBoard.buildDropIndex`):
    /// `item_id → [CbDrop]`, built on first use and cached. Adena (item 57) is
    /// excluded (Java `BLOCK_ID`); each item's list is sorted by NPC level.
    pub fn drop_index(&self) -> &HashMap<i32, Vec<CbDrop>> {
        self.drop_index.get_or_init(|| {
            const ADENA_ID: i32 = 57;
            let mut index: HashMap<i32, Vec<CbDrop>> = HashMap::new();
            let add = |index: &mut HashMap<i32, Vec<CbDrop>>, t: &NpcTemplate, d: &DropHolder, chance: f64, is_spoil: bool| {
                if d.item_id == ADENA_ID {
                    return;
                }
                index.entry(d.item_id).or_default().push(CbDrop {
                    item_id: d.item_id,
                    npc_id: t.id,
                    npc_level: t.level,
                    min: d.min,
                    max: d.max,
                    chance,
                    is_spoil,
                    is_raid: t.is_raid(),
                });
            };
            // Same insertion order as Java: grouped death drops (group chance
            // folded in), then ungrouped death drops, then spoil.
            for t in self.by_id.values() {
                for group in &t.drop_groups {
                    let group_chance = group.chance / 100.0;
                    for d in &group.items {
                        add(&mut index, t, d, d.chance * group_chance, false);
                    }
                }
                for d in &t.drop_list_death {
                    add(&mut index, t, d, d.chance, false);
                }
                for d in &t.drop_list_spoil {
                    add(&mut index, t, d, d.chance, true);
                }
            }
            for list in index.values_mut() {
                list.sort_by_key(|d| d.npc_level);
            }
            index
        })
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
        race: None,
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
        drop_list_spoil: Vec::new(),
        rhand: 0,
        lhand: 0,
        attackable: true,
        targetable: true,
        talkable: true,
        show_name: true,
        can_move: true,
        random_walk: false,
        random_animation: true,
        is_aggressive: false,
        ai_type: AiType::Fighter,
        min_skill_chance: 7,
        max_skill_chance: 15,
        clans: Vec::new(),
        ignore_clan_npc_ids: Vec::new(),
        minions: Vec::new(),
        aggro_range: 0,
        clan_help_range: 0,
        skill_list: Vec::new(),
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
    let mut in_clan = false;
    let mut in_ignore_npc_id = false;
    // `<parameters>` can nest `<minions><npc .../></minions>` (and skill/param
    // rows). Those inner `<npc>` tags are minion references, NOT new templates:
    // treating them as template starts overwrites the parent's `cur` and drops
    // its whole body (stats + dropLists come after `<parameters>`). Suppress
    // all `<npc>` start/end handling while inside this block.
    let mut in_parameters = false;
    // `<skillList>` scope: only the `<skill id level/>` rows here are template
    // skills (Java `NpcTemplate._skills`). `<parameters>` also carries `<skill
    // name=.. id level/>` AI holders, which are NOT `getSkills()` — this scope
    // flag keeps them out.
    let mut in_skill_list = false;
    // `<minions>` scope inside `<parameters>` — `<parameters>` also carries
    // unrelated `<npc>`-shaped rows, so the escort list needs its own flag.
    let mut in_minions = false;
    // `<race>` text scope (Java `NpcTemplate.setRace`).
    let mut in_race = false;

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
                } else if in_clan {
                    if let Some(t) = cur.as_mut() {
                        let name = String::from_utf8_lossy(&text).trim().to_string();
                        if !name.is_empty() {
                            t.clans.push(name);
                        }
                    }
                } else if in_ignore_npc_id {
                    if let Some(t) = cur.as_mut() {
                        if let Ok(v) = String::from_utf8_lossy(&text).trim().parse() {
                            t.ignore_clan_npc_ids.push(v);
                        }
                    }
                } else if in_race {
                    if let Some(t) = cur.as_mut() {
                        t.race = parse_race(String::from_utf8_lossy(&text).trim());
                    }
                }
                continue;
            }
            Event::End(e) => {
                match e.name().as_ref().to_ascii_lowercase().as_slice() {
                    b"parameters" => in_parameters = false,
                    b"minions" => in_minions = false,
                    b"skilllist" => in_skill_list = false,
                    // A minion's `</npc>` inside `<parameters>` must not flush
                    // the parent template.
                    b"npc" if !in_parameters => {
                        if let Some(t) = cur.take() {
                            finish_template(t, out);
                        }
                    }
                    b"attribute" => in_attribute = false,
                    b"corpsetime" => in_corpse_time = false,
                    b"race" => in_race = false,
                    b"clan" => in_clan = false,
                    b"ignorenpcid" => in_ignore_npc_id = false,
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
                    b"parameters" => in_parameters = !self_closing,
                    b"skilllist" => in_skill_list = !self_closing,
                    // `<skillList><skill id level/>` — a template skill. (The
                    // `name`-tagged `<parameters><skill>` rows never reach here:
                    // `in_skill_list` is false inside `<parameters>`.)
                    b"skill" if in_skill_list => {
                        if let (Some(t), Some(id)) = (cur.as_mut(), attr_i32(&e, b"id")) {
                            let level = attr_i32(&e, b"level").unwrap_or(1);
                            t.skill_list.push((id, level));
                        }
                    }
                    // Minion references inside `<parameters>` are not templates
                    // — but they *are* this NPC's escort list, so record them
                    // on the parent before skipping the template handling.
                    b"npc" if in_parameters => {
                        if in_minions {
                            if let (Some(t), Some(id)) = (cur.as_mut(), attr_i32(&e, b"id")) {
                                t.minions.push(MinionHolder {
                                    npc_id: id,
                                    count: attr_i32(&e, b"count").unwrap_or(1),
                                });
                            }
                        }
                        continue;
                    }
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
                            if let Some(v) = attr_bool(&e, b"randomAnimation") {
                                t.random_animation = v;
                            }
                        }
                    }
                    b"ai" => {
                        if let Some(t) = cur.as_mut() {
                            if let Some(v) = attr_bool(&e, b"isAggressive") {
                                t.is_aggressive = v;
                            }
                            set_i32(&e, b"aggroRange", &mut t.aggro_range);
                            set_i32(&e, b"clanHelpRange", &mut t.clan_help_range);
                            if let Some(v) = attr_str(&e, b"type") {
                                t.ai_type = AiType::parse(&v);
                            }
                            set_i32(&e, b"minSkillChance", &mut t.min_skill_chance);
                            set_i32(&e, b"maxSkillChance", &mut t.max_skill_chance);
                        }
                    }
                    b"corpsetime" => in_corpse_time = !self_closing,
                    b"race" => in_race = !self_closing,
                    b"minions" => in_minions = !self_closing,
                    b"clan" => in_clan = !self_closing,
                    b"ignorenpcid" => in_ignore_npc_id = !self_closing,
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
                        } else if drop_scope == DropScope::Spoil {
                            if let Some(t) = cur.as_mut() {
                                t.drop_list_spoil.push(holder);
                            }
                        }
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

/// `<race>` → `Race` ordinal — playable races *and* the creature-category
/// values (`UNDEAD`, `BEAST`, …), via the same shared enum
/// [`crate::enums::Race`] uses for `Player.race`/`respawn.xml`. A playable
/// race here still never equals a monster's, and vice versa — the ordinals
/// don't overlap — so extending past the original six playable-only values
/// doesn't change any existing `npc.race == player.race` comparison.
fn parse_race(text: &str) -> Option<i32> {
    let upper = text.to_ascii_uppercase();
    // "DARKELF" (no underscore) isn't a real `<race>` value on this dist, but
    // predates this function's move to `Race::from_name` — kept for safety.
    let normalized = if upper == "DARKELF" { "DARK_ELF" } else { upper.as_str() };
    crate::enums::Race::from_name(normalized).map(|r| r.ordinal())
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

        // Starting-village Gremlin (18342): has aggroRange 450 but no
        // isAggressive flag — must stay passive (the aggroRange-implies-
        // aggressive bug made newbie mobs hostile).
        let sg = data.get(18342).expect("npc 18342");
        assert_eq!(sg.aggro_range, 450);
        assert!(!sg.is_aggressive);
        // Gora Werewolf (20012) is the first mob with isAggressive="true".
        assert!(data.get(20012).expect("npc 20012").is_aggressive);

        // Goblin (20003): drop list + aggro range, hand-checked from the XML.
        let goblin = data.get(20003).expect("npc 20003");
        assert_eq!(goblin.aggro_range, 450);
        assert!(!goblin.is_aggressive);
        assert_eq!(goblin.drop_list_death.len(), 9);
        let adena = goblin.drop_list_death.iter().find(|d| d.item_id == 57).expect("adena line");
        assert_eq!((adena.min, adena.max), (13, 30));
        assert_eq!(adena.chance, 70.0);
        assert!(goblin.drop_groups.is_empty());
        // <spoil> lines land in `drop_list_spoil` (Goblin 20003: Magic Ring
        // 76.92%, Charcoal 12.1%) — a separate list from the 9 death drops.
        assert_eq!(goblin.drop_list_spoil.len(), 2);
        let magic_ring = goblin.drop_list_spoil.iter().find(|d| d.item_id == 116).expect("spoil Magic Ring");
        assert_eq!(magic_ring.chance, 76.92);

        // <corpseTime> element text (npc 103, Holiday Santa); absent = None.
        assert_eq!(data.get(103).unwrap().corpse_time, Some(3));
        assert_eq!(g.corpse_time, None);

        // getTemplateByName (admin spawn "Id/Name" input): case-insensitive
        // exact-name match. "Gremlin" is a real name; several ids share it, so
        // (like Java) which one is returned is unspecified — assert on the name.
        assert_eq!(data.get_by_name("Gremlin").map(|t| t.name.as_str()), Some("Gremlin"));
        assert_eq!(data.get_by_name("gremlin").map(|t| t.name.as_str()), Some("Gremlin"));
        assert!(data.get_by_name("NoSuchNpcName").is_none());

        // getAllMonstersOfLevel (spawn-by-level "List" buttons): exact level +
        // Monster type, id-sorted. Gremlin (20001) is level 1, type Monster.
        let lvl1 = data.monsters_of_level(1);
        assert!(lvl1.iter().any(|t| t.id == 20001), "Gremlin should be a level-1 Monster");
        assert!(lvl1.iter().all(|t| t.level == 1 && t.type_name.eq_ignore_ascii_case("Monster")));
        assert!(lvl1.windows(2).all(|w| w[0].id <= w[1].id), "monsters_of_level sorted by id");
        // Folk 100 (Thomas D. Turkey) is not a monster of any level.
        assert!(!data.monsters_of_level(80).iter().any(|t| t.id == 100));

        // getAllNpcStartingWith (A–Z letter buttons): Folk type + name prefix.
        let t_folk = data.folk_starting_with("T");
        assert!(t_folk.iter().any(|t| t.id == 100), "Thomas D. Turkey should list under 'T'");
        assert!(t_folk.iter().all(|t| t.type_name.eq_ignore_ascii_case("Folk") && t.name.starts_with('T')));
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
    fn drop_index_inverts_drops_and_excludes_adena() {
        let data = NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        let index = data.drop_index();
        // Adena (57) is on the block list — never indexed.
        assert!(!index.contains_key(&57), "adena excluded from the drop index");
        // Goblin (20003) spoils Magic Ring (116) — a spoil entry maps back to it.
        let ring = index.get(&116).expect("Magic Ring indexed");
        let goblin_spoil = ring
            .iter()
            .find(|d| d.npc_id == 20003 && d.is_spoil)
            .expect("goblin spoils Magic Ring");
        assert_eq!(goblin_spoil.npc_level, data.get(20003).unwrap().level);
        // Each item's list is sorted by NPC level (ascending).
        assert!(ring.windows(2).all(|w| w[0].npc_level <= w[1].npc_level), "sorted by level");
        // Cached: a second call returns the same populated map.
        assert!(std::ptr::eq(index, data.drop_index()), "drop index is built once and cached");
    }

    #[test]
    fn status_flags_parse() {
        let data = NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // Npc 101: <status attackable="false" canMove="false"/>.
        let t = data.get(101).expect("npc 101");
        assert!(!t.attackable);
        assert!(!t.can_move);
    }

    /// An NPC whose `<parameters>` nests `<minions><npc .../></minions>` must
    /// still parse fully: the minion references are not templates, so they must
    /// not overwrite the parent's `cur` (which used to swallow the parent's
    /// whole body — stats *and* the `<dropLists>` that follow `<parameters>`).
    /// Raid boss 3404 (Tracker Captain Sharuk) declares minions 3405/3406 then
    /// a 15-line drop list.
    #[test]
    fn npc_with_nested_minions_keeps_its_body_and_drops() {
        let data = NpcData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

        let boss = data.get(3404).expect("npc 3404 must load despite nested minions");
        assert_eq!(boss.type_name, "RaidBoss");
        assert_eq!(boss.level, 23);
        assert_eq!(boss.drop_list_death.len(), 15, "all 15 death drops parsed");
        assert!(boss.drop_list_death.iter().any(|d| d.item_id == 955), "D-grade enchant scroll drop");

        // The minion ref must not have created/corrupted 3405: its own later
        // `<npc>` block is the real definition (a level-22 Monster).
        let minion = data.get(3405).expect("npc 3405 real block");
        assert_eq!(minion.type_name, "Monster");
        assert_eq!(minion.level, 22);
    }
}

#[cfg(test)]
mod clan_tests {
    use super::*;

    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

    /// The `<minions>` parse, against the real datapack.
    #[test]
    fn parses_minions_from_dist() {
        let data = NpcData::load_from(DIST);
        let leaders = data.all().filter(|t| !t.minions.is_empty()).count();
        let entries: usize = data.all().map(|t| t.minions.len()).sum();
        println!("MINION LEADERS={leaders} ENTRIES={entries}");
        // 467 `<minions>` *blocks* exist in the XML but they sit on 460
        // distinct NPCs — a handful declare many groups each (25100 has 28,
        // 25200 has 29, 22602 has 15). The entry count is the check that
        // matters: every one of the 962 `<npc>` rows is captured.
        assert_eq!(leaders, 460, "expected the dist's 460 minion-leading NPCs");
        assert_eq!(entries, 962, "expected the dist's 962 minion entries");

        // Tracker Captain Sharuk 3404 declares 3402 x3 and 3403 x1.
        let boss = data.get(3404).expect("3404 loads");
        assert!(!boss.minions.is_empty(), "3404 must carry its escort");
    }

    /// The `<clanList>` parse, against the real datapack — a fixture would
    /// agree with whatever the parser does.
    #[test]
    fn parses_clans_and_guards_from_dist() {
        let data = NpcData::load_from(DIST);
        let with_clans = data.all().filter(|t| !t.clans.is_empty()).count();
        let guards = data.all().filter(|t| t.is_guard()).count();
        let ignores = data.all().filter(|t| !t.ignore_clan_npc_ids.is_empty()).count();
        println!("CLANS={with_clans} GUARDS={guards} IGNORES={ignores}");
        assert!(with_clans > 2000, "expected thousands of faction NPCs, got {with_clans}");
        assert!(guards > 100, "expected ~186 Guard templates, got {guards}");
        assert!(ignores > 0, "expected some ignoreNpcId lists, got {ignores}");

        // Cave Servant 20236 sits in a clanList that also carries ignoreNpcIds.
        if let Some(t) = data.get(20236) {
            assert!(!t.clans.is_empty(), "20236 should carry a clan");
        }
    }
}

#[cfg(test)]
mod race_tests {
    use super::*;

    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

    /// `<race>` parses to the `Race` ordinal for both the five playable races
    /// (the Newbie Guides' own-race gate reads this field) and the
    /// creature-category values (G19's `AttackTrait`/`*_WEAKNESS` reads
    /// those) — the ordinals never collide, so a monster's race still never
    /// equals a player's.
    #[test]
    fn parses_playable_races_from_dist() {
        let data = NpcData::load_from(DIST);
        // One Newbie Guide per starter village. Id order is *not* race
        // order: 30601 is the Dwarven village (DWARF=4), 30602 the Orc one
        // (ORC=3).
        for (npc_id, race) in [(30598, 0), (30599, 1), (30600, 2), (30601, 4), (30602, 3)] {
            let t = data.get(npc_id).unwrap_or_else(|| panic!("{npc_id} loads"));
            assert_eq!(t.race, Some(race), "npc {npc_id} race");
        }
        // A monster's `<race>UNDEAD</race>` now parses too (24), and it's
        // still never equal to any playable race's ordinal (0-6).
        assert_eq!(
            data.get(20015).and_then(|t| t.race),
            Some(crate::enums::Race::Undead.ordinal()),
            "undead parses to its own Race ordinal"
        );
    }
}
