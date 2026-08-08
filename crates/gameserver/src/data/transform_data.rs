//! Port of `data/xml/TransformData` (`data/stats/transformations/*.xml`, 174
//! templates on this dist). A transform swaps the player's model (display id),
//! collision, move speed, and grants the template's transform skills.
//!
//! Scope: display id (always == the transform id here; no template carries a
//! `displayId` attribute), the `FLYING` flag, per-gender `collision` /
//! `moving` (walk+run) / `skills`, the `<actions>` action-bar list
//! (`ExBasicActionList`), and the `<base>` combat overrides.
//!
//! **Deliberately not ported: `<stats>`, `<defense>`, `<magicDefense>` and
//! `<levels>`** — the `SKIP(census)` sits on the parser's element match, where
//! the arms for them would go. A reachability census over this
//! dist (2026-08-08) found that of the 174 templates a player can enter
//! exactly **two**: transform 105 (the Rabbit event's `applyEffects`, in
//! `custom/events/Rabbits`) and transform 20008 (a 30-day mount from
//! `AttendanceRewards.xml`). Neither carries any of those four blocks, so
//! nothing that reads them would ever observe a value.
//!
//! What makes the other 172 unreachable, since a "no route" claim is the kind
//! that keeps turning out wrong here:
//! - The 32-entry `transformSkillTree.xml` has exactly **one** root entry,
//!   skill 617 Transform Onyx Beast, gated on item 9648 "Transformation
//!   Sealbook: Onyx Beast". Items 9648-9655 appear nowhere in the datapack
//!   except that tree — no drop, buylist, multisell, recipe or quest grant.
//!   Every other tree entry carries `<preRequisiteSkill id="617">`, and Java
//!   *does* enforce it (`RequestAcquireSkill:608`), so the whole tree is dead
//!   behind one unobtainable book. (The stones 10297-10305 that gate the
//!   raid-boss forms *are* droppable — but they are useless without 617.)
//! - Every remaining route is an item whose `<skills>` grants a transform
//!   skill, and each of those items is itself unobtainable on this dist.
//! - **No NPC** carries a transform-granting skill (0 of 306).
//!
//! `<base>` is ported because both live templates carry it, and for a
//! `NON_COMBAT`/`RIDING_MODE` transform Java keeps the transform's base
//! instead of the weapon's — see [`TransformBase`].

use std::collections::{HashMap, HashSet};

use crate::data::xml::attr_strict as attr;
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const TRANSFORM_DIR: &str = "data/stats/transformations";

/// Java `TransformType`. Only the distinction the stat pipeline draws is
/// load-bearing: `COMBAT` and `MODE_CHANGE` let the equipped **weapon**
/// overwrite the transform's `<base>` values, every other type keeps the
/// transform's (Java `IStatFunction.calcWeaponBaseValue`, the `else if` at
/// line 76 — note the branch reads as "weapon wins", so the forms that keep
/// their own base are the ones the condition *excludes*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransformKind {
    Combat,
    ModeChange,
    NonCombat,
    RidingMode,
    PureStat,
    Flying,
    Cursed,
    #[default]
    Unknown,
}

impl TransformKind {
    fn parse(s: Option<&str>) -> Self {
        match s {
            Some("COMBAT") => Self::Combat,
            Some("MODE_CHANGE") => Self::ModeChange,
            Some("NON_COMBAT") => Self::NonCombat,
            Some("RIDING_MODE") => Self::RidingMode,
            Some("PURE_STAT") => Self::PureStat,
            Some("FLYING") => Self::Flying,
            Some("CURSED") => Self::Cursed,
            _ => Self::Unknown,
        }
    }

    /// Java's `calcWeaponBaseValue` gate: `true` when the equipped weapon
    /// replaces the transform's `<base>` value rather than the other way round.
    ///
    /// Java's sibling `isStance()` (`PURE_STAT`) gates `calcWeaponPlusBaseValue`
    /// instead, which reads accuracy/evasion/shield keys — all of them supplied
    /// by `<stats>`, which this dist puts on no reachable template. Adding the
    /// predicate here with nothing to consume it would be a registry line, not
    /// a port, so it is left out until `<stats>` earns its way in.
    pub fn weapon_overrides_base(self) -> bool {
        matches!(self, Self::Combat | Self::ModeChange)
    }
}

/// `<base>` — the transform's own weapon-ish base line. Java folds each
/// attribute into the template's stat map (`TransformTemplate:89-105`), where
/// it stands in for the class base *and*, for a non-`COMBAT`/`MODE_CHANGE`
/// form, for the equipped weapon.
///
/// Every attribute defaults to Java's `set.getDouble(name, 0)` / `getInt`, so
/// an absent attribute reads as 0 — but a 0 here means "no override" for the
/// fields that stand in for a weapon, which is why each is an `Option`: Java
/// only ever `addStats`-es the key when the attribute is present.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformBase {
    /// `range` → `Stat.PHYSICAL_ATTACK_RANGE`.
    pub attack_range: Option<f64>,
    /// `attackSpeed` → `Stat.PHYSICAL_ATTACK_SPEED`.
    pub attack_speed: Option<f64>,
    /// `critRate` → `Stat.CRITICAL_RATE`.
    pub crit_rate: Option<f64>,
    /// `pAtk` → `Stat.PHYSICAL_ATTACK`.
    pub p_atk: Option<f64>,
    /// `mAtk` → `Stat.MAGIC_ATTACK`.
    pub m_atk: Option<f64>,
    /// `randomDamage` → `Stat.RANDOM_DAMAGE`.
    pub random_damage: Option<f64>,
    /// `attackType` → Java `getBaseAttackType`, the weapon type the attack
    /// animation and `Formulas:1513`'s hit-condition bonus read.
    pub attack_type: Option<String>,
}

/// Per-gender transform template (Java `TransformTemplate`), narrowed to the
/// consumed fields.
#[derive(Debug, Clone, Default)]
pub struct TransformTemplate {
    pub collision_radius: f64,
    pub collision_height: f64,
    /// `<moving run=…>` — the transform's run speed, replacing the player's
    /// while transformed. `None` when the template omits `<moving>`.
    pub run_spd: Option<f64>,
    pub walk_spd: Option<f64>,
    /// `<skills>` granted on transform (Java `addTransformSkill`), `(id, level)`.
    pub skills: Vec<(i32, i32)>,
    /// `<base>` — see [`TransformBase`]. `None` when the template omits it
    /// (7 of the 174 do).
    pub base: Option<TransformBase>,
    /// `<actions>` — the action-bar id list sent as `ExBasicActionList` on
    /// transform (Java `Transform.onTransform`'s `hasBasicActionList()`
    /// branch). All 174 templates carry one; untransforming restores the
    /// default list (`ExBasicActionList.STATIC_PACKET`).
    pub actions: Vec<i32>,
}

/// A transform (`Transform`): id, display id, flying flag, and the two gender
/// templates.
#[derive(Debug, Clone)]
pub struct Transform {
    pub id: i32,
    /// Java `_displayId = getInt("displayId", _id)`; no template on this dist
    /// sets `displayId`, so this equals `id`.
    pub display_id: i32,
    pub flying: bool,
    /// Java `Transform.isRiding()` — `type="RIDING_MODE"` (34 of them on this
    /// dist: the horse/bike rides). `AllowRideMountsDuringSiege = False` makes
    /// a siege zone untransform them, the same way it dismounts a strider.
    pub riding: bool,
    /// Java `Transform.canSwim()` — `can_swim="1"`. Entering a `WaterZone`
    /// cancels any transform that lacks it (`WaterZone.onEnter`'s
    /// `checkTransformed(transform -> !transform.canSwim())` →
    /// `stopTransformation(true)`), which on this dist is 157 of the 174
    /// templates: nearly every transform pops the moment you go under.
    pub can_swim: bool,
    /// Java `Transform.isCombat()` — `type="COMBAT"` (89 of the 174 templates
    /// on this dist). Read by the AI's "while flying there is no move to cast"
    /// refusal: a player in a **non**-combat form who would have to walk into
    /// range is refused instead of walked.
    pub combat: bool,
    /// The raw `type=` attribute. `combat`/`riding`/`flying` above are the
    /// long-standing derived flags; the stat pipeline needs the full value to
    /// tell `MODE_CHANGE` from the other non-`COMBAT` forms.
    pub kind: TransformKind,
    pub male: TransformTemplate,
    pub female: TransformTemplate,
}

impl Transform {
    /// The gender-appropriate template (Java `getTemplate(creature)`).
    pub fn template(&self, is_female: bool) -> &TransformTemplate {
        if is_female { &self.female } else { &self.male }
    }
}

#[derive(Debug, Default)]
pub struct TransformData {
    by_id: HashMap<i32, Transform>,
    /// Every skill id any transform template grants (either gender). Transform
    /// skills are session-only in Java (`Player._transformSkills`, which
    /// `storeSkills` never writes), so the persistence boundary filters them
    /// with [`Self::is_transform_skill`].
    skill_ids: HashSet<i32>,
}

impl TransformData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_id = HashMap::new();
        if let Ok(dir) = std::fs::read_dir(format!("{file_path}{TRANSFORM_DIR}")) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path)
                    && let Some(t) = parse(&content)
                {
                    by_id.insert(t.id, t);
                }
            }
        }
        info!("TransformData: Loaded {} transforms.", by_id.len());
        let skill_ids = by_id
            .values()
            .flat_map(|t: &Transform| t.male.skills.iter().chain(t.female.skills.iter()))
            .map(|&(id, _)| id)
            .collect();
        Self { by_id, skill_ids }
    }

    pub fn empty() -> Self {
        Self {
            by_id: HashMap::new(),
            skill_ids: HashSet::new(),
        }
    }

    pub fn get(&self, id: i32) -> Option<&Transform> {
        self.by_id.get(&id)
    }

    /// Whether `id` is a transform-granted skill (Dismount 839, Dissonance
    /// 5437, …). These live in the `SkillBook` only while the transform is
    /// active and must never reach `character_skills` — a row that leaks there
    /// re-applies the skill's passives (e.g. Dissonance's Accuracy -50) on
    /// every login with no transform backing it.
    pub fn is_transform_skill(&self, id: i32) -> bool {
        self.skill_ids.contains(&id)
    }
}

/// Parse one `<transform>` file. Returns `None` if the id is missing.
fn parse(content: &str) -> Option<Transform> {
    let mut reader = Reader::from_str(content);
    let mut id = None;
    let mut flying = false;
    let mut riding = false;
    let mut combat = false;
    let mut can_swim = false;
    let mut kind = TransformKind::Unknown;
    let mut male = TransformTemplate::default();
    let mut female = TransformTemplate::default();
    // 0 = male, 1 = female; which gender block we're inside.
    let mut gender = 0usize;
    let mut in_skills = false;
    // `<actions>` holds its ids as whitespace-separated *text*, not attributes,
    // so the text event that follows the open tag has to be routed to the
    // gender block that opened it.
    let mut in_actions = false;

    let handle =
        |e: &quick_xml::events::BytesStart, tmpl: &mut TransformTemplate, in_skills: &mut bool| {
            // SKIP(census): `<stats>` (106 templates), `<defense>` (106),
            // `<magicDefense>` (106) and `<levels>` (60) are read by nothing
            // and parsed by nothing, deliberately. Of the 174 templates only
            // two are enterable by a player on this dist — 105 and 20008 — and
            // neither carries any of the four; the module header above records
            // the routes and why each of the other 172 is dead. Parsing them
            // without a consumer would be a registry line, not a port. Redo
            // the census before adding them: the answer changes the moment the
            // datapack gains a source for item 9648.
            match e.name().as_ref() {
                b"base" => {
                    let num = |k: &str| attr(e, k).and_then(|s| s.trim().parse::<f64>().ok());
                    tmpl.base = Some(TransformBase {
                        attack_range: num("range"),
                        attack_speed: num("attackSpeed"),
                        crit_rate: num("critRate"),
                        p_atk: num("pAtk"),
                        m_atk: num("mAtk"),
                        random_damage: num("randomDamage"),
                        attack_type: attr(e, "attackType"),
                    });
                }
                b"collision" => {
                    if let Some(r) = attr(e, "radius").and_then(|s| s.parse().ok()) {
                        tmpl.collision_radius = r;
                    }
                    if let Some(h) = attr(e, "height").and_then(|s| s.parse().ok()) {
                        tmpl.collision_height = h;
                    }
                }
                b"moving" => {
                    tmpl.run_spd = attr(e, "run").and_then(|s| s.parse().ok());
                    tmpl.walk_spd = attr(e, "walk").and_then(|s| s.parse().ok());
                }
                b"skills" => *in_skills = true,
                b"skill" if *in_skills => {
                    if let (Some(sid), level) = (
                        attr(e, "id").and_then(|s| s.parse().ok()),
                        attr(e, "level").and_then(|s| s.parse().ok()).unwrap_or(1),
                    ) {
                        tmpl.skills.push((sid, level));
                    }
                }
                _ => {}
            }
        };

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"transform" => {
                    id = attr(&e, "id").and_then(|s| s.parse().ok());
                    let ty = attr(&e, "type");
                    kind = TransformKind::parse(ty.as_deref());
                    flying = ty.as_deref() == Some("FLYING");
                    riding = ty.as_deref() == Some("RIDING_MODE");
                    combat = ty.as_deref() == Some("COMBAT");
                    // `set.getInt("can_swim", 0) == 1` — absent means "no".
                    can_swim = attr(&e, "can_swim").as_deref() == Some("1");
                }
                b"Male" => {
                    gender = 0;
                    in_skills = false;
                }
                b"Female" => {
                    gender = 1;
                    in_skills = false;
                }
                b"actions" => in_actions = true,
                _ => {
                    let tmpl = if gender == 0 { &mut male } else { &mut female };
                    handle(&e, tmpl, &mut in_skills);
                }
            },
            Ok(Event::Text(t)) if in_actions => {
                let tmpl = if gender == 0 { &mut male } else { &mut female };
                if let Ok(text) = t.unescape() {
                    tmpl.actions.extend(
                        text.split_whitespace()
                            .filter_map(|s| s.parse::<i32>().ok()),
                    );
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"actions" => in_actions = false,
            Ok(Event::End(e)) if e.name().as_ref() == b"skills" => in_skills = false,
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let id = id?;
    Some(Transform {
        id,
        display_id: id,
        flying,
        riding,
        combat,
        kind,
        can_swim,
        male,
        female,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `<actions>` carries its ids as element **text**, and each gender block
    /// has its own — so the text event has to be routed to the block that
    /// opened it. Getting that wrong silently gives the female template the
    /// male list (or an empty one), which no downstream assertion would catch.
    #[test]
    fn actions_and_base_parse_per_gender() {
        let xml = r#"
        <list>
          <transform id="4242" type="NON_COMBAT" can_swim="0">
            <Male>
              <common>
                <base range="20" attackSpeed="300" attackType="SWORD"
                      critRate="5" mAtk="7" pAtk="9" randomDamage="10" />
                <collision radius="5" height="4.5" />
              </common>
              <actions>1 2 3</actions>
            </Male>
            <Female>
              <common>
                <base range="21" attackSpeed="301" attackType="FIST"
                      critRate="6" mAtk="8" pAtk="10" randomDamage="11" />
                <collision radius="6" height="5.5" />
              </common>
              <actions>7 8 9 10</actions>
            </Female>
          </transform>
        </list>"#;
        let t = parse(xml).expect("parses");
        assert_eq!(t.kind, TransformKind::NonCombat);
        assert!(!t.kind.weapon_overrides_base(), "NON_COMBAT keeps its base");

        assert_eq!(t.male.actions, vec![1, 2, 3]);
        assert_eq!(
            t.female.actions,
            vec![7, 8, 9, 10],
            "the female block gets its own list, not the male one"
        );

        let m = t.male.base.as_ref().expect("male <base>");
        assert_eq!(m.p_atk, Some(9.0));
        assert_eq!(m.m_atk, Some(7.0));
        assert_eq!(m.crit_rate, Some(5.0));
        assert_eq!(m.attack_speed, Some(300.0));
        assert_eq!(m.attack_range, Some(20.0));
        assert_eq!(m.random_damage, Some(10.0));
        assert_eq!(m.attack_type.as_deref(), Some("SWORD"));

        let f = t.female.base.as_ref().expect("female <base>");
        assert_eq!(f.p_atk, Some(10.0), "female base is the female block's");
        assert_eq!(f.attack_type.as_deref(), Some("FIST"));
    }

    /// A template with no `<base>` must report `None`, not a zero-filled block:
    /// `Some(all zeroes)` would make `p_atk` 0 for every form that carries no
    /// base, which is a silent nerf rather than "no override".
    #[test]
    fn a_template_without_base_reports_none() {
        let xml = r#"
        <list>
          <transform id="4243" type="COMBAT">
            <Male><common><collision radius="5" height="4.5" /></common></Male>
          </transform>
        </list>"#;
        let t = parse(xml).expect("parses");
        assert!(t.male.base.is_none(), "absent <base> stays None");
        assert!(t.male.actions.is_empty(), "absent <actions> stays empty");
        assert!(t.kind.weapon_overrides_base(), "COMBAT lets the weapon win");
    }
}
