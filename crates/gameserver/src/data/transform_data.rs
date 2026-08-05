//! Port of `data/xml/TransformData` (`data/stats/transformations/*.xml`, 174
//! templates on this dist). A transform swaps the player's model (display id),
//! collision, move speed, and grants the template's transform skills.
//!
//! Scope: the fields the runtime consumes — display id (always == the transform
//! id here; no template carries a `displayId` attribute), the `FLYING` flag,
//! and per-gender `collision` / `moving` (walk+run) / `skills`. The deeper
//! `<base>`/`<stats>`/`<defense>`/`<magicDefense>` combat overrides, the
//! action-list swap, and additional-item inventory blocks are not applied yet
//! (documented TODO — the visual model, speed, collision and skills are the
//! parts the admin `//transform`/`//ride_horse` path needs).

use std::collections::{HashMap, HashSet};

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const TRANSFORM_DIR: &str = "data/stats/transformations";

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

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}

/// Parse one `<transform>` file. Returns `None` if the id is missing.
fn parse(content: &str) -> Option<Transform> {
    let mut reader = Reader::from_str(content);
    let mut id = None;
    let mut flying = false;
    let mut riding = false;
    let mut combat = false;
    let mut can_swim = false;
    let mut male = TransformTemplate::default();
    let mut female = TransformTemplate::default();
    // 0 = male, 1 = female; which gender block we're inside.
    let mut gender = 0usize;
    let mut in_skills = false;

    let handle =
        |e: &quick_xml::events::BytesStart, tmpl: &mut TransformTemplate, in_skills: &mut bool| {
            match e.name().as_ref() {
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
                    let kind = attr(&e, "type");
                    flying = kind.as_deref() == Some("FLYING");
                    riding = kind.as_deref() == Some("RIDING_MODE");
                    combat = kind.as_deref() == Some("COMBAT");
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
                _ => {
                    let tmpl = if gender == 0 { &mut male } else { &mut female };
                    handle(&e, tmpl, &mut in_skills);
                }
            },
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
        can_swim,
        male,
        female,
    })
}
