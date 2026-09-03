use super::CondScope;
use super::EffectScope;
use super::LeveledValues;
use super::ParsedCondition;
use super::ParsedEffect;
use super::ParsedSkills;
use super::RangedRow;
use super::SkillGaps;
use super::effect_level_attrs;
use super::finalize_skill;
use super::ranged_bounds;
use super::record_dropped_scope;
use crate::data::xml;
use crate::data::xml::attr_f64;
use crate::data::xml::attr_i32;
use crate::data::xml::attr_i64;
use crate::data::xml::attr_str;
use crate::model::skill::effects::RestorationGroup;
use crate::model::skill::effects::RestorationItem;
use quick_xml::events::Event;
use std::collections::BTreeSet;
use std::collections::HashMap;
use tracing::info;
use tracing::warn;

pub(crate) fn parse_str(content: &str, out: &mut ParsedSkills) {
    // Current `<skill>` being built (id/name/toLevel + the generic field map).
    let mut skill_id = -1;
    let mut skill_name = String::new();
    let mut to_level = 1;
    let mut values: LeveledValues = HashMap::new();
    let mut cur_field = String::new();
    let mut pending_level: i32 = 0;

    // Effects collected for the current skill: (xml name, per-level params
    // keyed by param name — `amount` for stat modifiers, `power` for the
    // instant damage/heal handlers —, mode, RestorationRandom groups).
    let mut effects: Vec<ParsedEffect> = Vec::new();
    // The current `<effect>`'s level-range attributes (Java `NamedParamInfo`).
    let mut cur_effect_levels: (Option<i32>, Option<i32>, Option<i32>, Option<i32>) =
        (None, None, None, None);
    let mut in_effects = false;
    let mut cur_scope = EffectScope::General;
    // Which `<*onditions>` block is open, for the coverage record: Java's three
    // `SkillConditionScope`s (`conditions`/`targetConditions`/
    // `passiveConditions`). `in_conditions` above stays GENERAL-only, since
    // that is the only block `op_exist_npc` may be read from.
    let mut cond_block: Option<String> = None;
    // Conditions collected for the current skill, all three scopes.
    let mut conditions: Vec<ParsedCondition> = Vec::new();
    let mut cur_cond_params: LeveledValues = HashMap::new();
    let mut cur_cond_sub_params: HashMap<String, Vec<RangedRow>> = HashMap::new();
    let mut cur_cond_lists: HashMap<String, Vec<String>> = HashMap::new();
    let mut cur_cond_name: Option<String> = None;
    let mut cur_cond_field = String::new();
    // Ranged `<value>` rows (fromLevel/fromSubLevel bounds) — collected raw
    // per skill field / effect param, resolved at finalize.
    let mut pending_range: Option<RangedRow> = None;
    let mut field_rows: HashMap<String, Vec<RangedRow>> = HashMap::new();
    let mut cur_effect_sub_params: HashMap<String, Vec<RangedRow>> = HashMap::new();
    let mut cur_effect_name: Option<String> = None;
    let mut cur_effect_params: LeveledValues = HashMap::new();
    let mut cur_effect_mode = String::from("DIFF");
    let mut cur_effect_field = String::new();
    // OR of `ArmorType::mask_bit`s from the current effect's `<armorType>`
    // list (`ConditionUsingItemType`); 0 = no armor condition. Reset per effect.
    let mut cur_effect_armor: u8 = 0;
    // OR of `WeaponType::mask_bit`s from the current effect's `<weaponType>`
    // list; 0 = no weapon condition. Reset per effect.
    let mut cur_effect_weapon: u32 = 0;

    // `RestorationRandom`'s `<items><item chance="30"><item id=".." count=".."
    // /></item></items>` shape doesn't fit the scalar/leveled-value model
    // above (a list of chance-weighted item groups), so it's tracked
    // separately: `cur_restoration_groups` accumulates finished groups for
    // the current `<effect>`, `cur_group_chance`/`cur_group_items` build the
    // group currently open.
    let mut cur_restoration_groups: Vec<RestorationGroup> = Vec::new();
    let mut cur_group_chance: f64 = 0.0;
    let mut cur_group_items: Vec<RestorationItem> = Vec::new();

    // Tag-name stack relative to `<skill>` (path[0] == "skill" once inside one).
    let mut path: Vec<String> = Vec::new();

    for event in xml::events(content) {
        match event {
            Event::Empty(e) => {
                // A param-less `<condition name="X" />` — by far the common
                // shape (`OpCanEscape`, `CanSummon`, `EquipShield`, …). No
                // Start/End pair fires for an `Empty`, so the coverage record
                // has to happen here as well or the census under-reports.
                if path.len() == 2
                    && e.name().as_ref() == b"condition"
                    && let Some(block) = &cond_block
                {
                    // No Start/End pair fires for an `Empty`, so the param-less
                    // condition has to be pushed here or it is lost — which is
                    // most of them (`CanSummon`, `EquipShield`, `OpCanEscape`…).
                    if let (Some(scope), Some(name)) =
                        (CondScope::from_xml(block), attr_str(&e, b"name"))
                    {
                        conditions.push(ParsedCondition {
                            scope,
                            name,
                            params: HashMap::new(),
                            sub_params: HashMap::new(),
                            lists: HashMap::new(),
                        });
                    }
                }
                // Self-closing leaf (e.g. an attribute-only tag with no text).
                // Not pushed onto `path` since no matching `End` event follows
                // — the one shape this loader reads here is `RestorationRandom`'s
                // inner `<item id=".." count=".."/>`, sitting right inside an
                // open group (`path` still at the group's depth, 5).
                if in_effects
                    && cur_effect_field == "items"
                    && path.len() == 5
                    && e.name().as_ref() == b"item"
                {
                    if let (Some(item_id), Some(count)) =
                        (attr_i32(&e, b"id"), attr_i64(&e, b"count"))
                    {
                        cur_group_items.push(RestorationItem {
                            item_id,
                            count,
                            min_enchant: attr_i32(&e, b"minEnchant").unwrap_or(0),
                            max_enchant: attr_i32(&e, b"maxEnchant").unwrap_or(0),
                        });
                    }
                } else if in_effects && path.len() == 2 && e.name().as_ref() == b"effect" {
                    // A param-less self-closing `<effect name="X" />` (Spoil,
                    // Sweeper, ConsumeBody, …). No Start/End pair fires for an
                    // `Empty` element, so capture it here with empty params —
                    // otherwise the effect is silently dropped and the skill
                    // becomes a no-op.
                    if let Some(effect_name) = attr_str(&e, b"name") {
                        if cur_scope == EffectScope::Other {
                            record_dropped_scope(
                                &mut out.gaps.borrow_mut(),
                                &cur_field,
                                Some(&effect_name),
                                skill_id,
                            );
                        }
                        let (from_level, to_level, from_sub_level, to_sub_level) =
                            effect_level_attrs(&e);
                        effects.push(ParsedEffect {
                            scope: cur_scope,
                            name: effect_name,
                            params: HashMap::new(),
                            sub_params: HashMap::new(),
                            mode: String::from("DIFF"),
                            groups: Vec::new(),
                            armor_condition: 0,
                            weapon_condition: 0,
                            from_level,
                            to_level,
                            from_sub_level,
                            to_sub_level,
                        });
                    }
                }
            }
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();

                // Coverage record (PLAN_G34 §S0) — every `<condition>` in any
                // of the three scopes, ported or not; `record_condition` drops
                // the one that *is* enforced. Sits ahead of the branch chain
                // because the chain only descends into the GENERAL block.

                if path.is_empty() {
                    if name != "skill" {
                        // The `<list>` document root (or anything else outside
                        // a `<skill>`) is not tracked — the stack is relative
                        // to `<skill>`, see its matching End guard below.
                        continue;
                    }
                    skill_id = attr_i32(&e, b"id").unwrap_or(-1);
                    skill_name = attr_str(&e, b"name").unwrap_or_default();
                    to_level = attr_i32(&e, b"toLevel").unwrap_or(1).max(1);
                    values.clear();
                    effects.clear();
                    conditions.clear();
                    cond_block = None;
                    in_effects = false;
                    field_rows.clear();
                    pending_range = None;
                } else if path.len() == 1 {
                    cur_field = name.clone();
                    // Any `<*Effects>` block opens the effect section; which one
                    // it is decides the scope every effect inside gets.
                    if name.ends_with("ffects") {
                        in_effects = true;
                        cur_scope = EffectScope::from_xml(&name);
                    } else if name.ends_with("onditions") {
                        cond_block = Some(name.clone());
                    }
                } else if path.len() == 2 && name == "value" && !in_effects && cond_block.is_none()
                {
                    pending_level = attr_i32(&e, b"level").unwrap_or(0);
                    pending_range = ranged_bounds(&e);
                } else if path.len() == 2 && cond_block.is_some() && name == "condition" {
                    cur_cond_name = attr_str(&e, b"name");
                } else if path.len() == 4 && cond_block.is_some() && name == "value" {
                    // `<condition><amount><value level="2">…` — same shape as an
                    // effect param one level shallower.
                    pending_level = attr_i32(&e, b"level").unwrap_or(0);
                    pending_range = ranged_bounds(&e);
                } else if path.len() == 3 && cond_block.is_some() {
                    cur_cond_field = name.clone();
                } else if path.len() == 2 && in_effects && name == "effect" {
                    if cur_scope == EffectScope::Other {
                        record_dropped_scope(
                            &mut out.gaps.borrow_mut(),
                            &cur_field,
                            attr_str(&e, b"name").as_deref(),
                            skill_id,
                        );
                    }
                    cur_effect_name = attr_str(&e, b"name");
                    cur_effect_levels = effect_level_attrs(&e);
                    cur_effect_params = HashMap::new();
                    cur_effect_mode = String::from("DIFF");
                    cur_effect_armor = 0;
                    cur_effect_weapon = 0;
                    cur_restoration_groups = Vec::new();
                    cur_effect_sub_params = HashMap::new();
                } else if path.len() == 3 && in_effects {
                    cur_effect_field = name.clone();
                } else if path.len() == 4 && in_effects && name == "value" {
                    pending_level = attr_i32(&e, b"level").unwrap_or(0);
                    pending_range = ranged_bounds(&e);
                } else if path.len() == 4
                    && in_effects
                    && cur_effect_field == "items"
                    && name == "item"
                {
                    // `RestorationRandom`'s outer `<item chance="30">` group tag.
                    cur_group_chance = attr_f64(&e, b"chance").unwrap_or(0.0);
                    cur_group_items = Vec::new();
                }
                path.push(name);
            }
            Event::Text(txt) => {
                let text = txt.unescape().unwrap_or_default();
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                if cond_block.is_some() {
                    match path.len() {
                        // `<condition><range>10</range>` — a plain scalar param,
                        // stored at level 0 so `value_at`'s fallback finds it at
                        // every level.
                        4 => {
                            cur_cond_params
                                .entry(cur_cond_field.clone())
                                .or_default()
                                .insert(0, text.to_string());
                        }
                        // `<condition><weaponType><item>DUAL</item>…` — the two
                        // list-valued params in this datapack.
                        5 if path.last().is_some_and(|t| t == "item") => {
                            cur_cond_lists
                                .entry(cur_cond_field.clone())
                                .or_default()
                                .push(text.to_string());
                        }
                        // `<condition><amount><value …>` — level table, with
                        // ranged rows resolved per (level, sub) at finalize.
                        5 if pending_range.is_some() => {
                            let mut row = pending_range.take().expect("checked");
                            row.text = text.to_string();
                            cur_cond_sub_params
                                .entry(cur_cond_field.clone())
                                .or_default()
                                .push(row);
                        }
                        5 => {
                            cur_cond_params
                                .entry(cur_cond_field.clone())
                                .or_default()
                                .insert(pending_level, text.to_string());
                        }
                        _ => {}
                    }
                } else if in_effects && cur_effect_field == "armorType" && path.len() == 5 {
                    // `<effect><armorType><item>MAGIC</item>...` — OR each armor
                    // kind's bit into the effect's condition mask.
                    cur_effect_armor |=
                        crate::data::item_data::ArmorType::from_name(text).mask_bit();
                } else if in_effects && cur_effect_field == "weaponType" && path.len() == 5 {
                    // `<effect><weaponType><item>BOW</item>...` — OR each weapon
                    // kind's bit into the effect's weapon-condition mask.
                    cur_effect_weapon |=
                        crate::data::item_data::WeaponType::from_name(text).mask_bit();
                } else if in_effects {
                    match path.len() {
                        4 if cur_effect_field == "mode" => {
                            cur_effect_mode = text.to_string();
                        }
                        // Directly under `<effect><param>SCALAR</param>`.
                        4 => {
                            cur_effect_params
                                .entry(cur_effect_field.clone())
                                .or_default()
                                .insert(0, text.to_string());
                        }
                        // `<effect><param><value fromLevel=… [fromSubLevel=…]>`
                        // — a ranged (possibly computed) row.
                        5 if pending_range.is_some() => {
                            let mut row = pending_range.take().expect("checked");
                            row.text = text.to_string();
                            cur_effect_sub_params
                                .entry(cur_effect_field.clone())
                                .or_default()
                                .push(row);
                        }
                        // `<effect><param><value level="N">...`
                        5 => {
                            cur_effect_params
                                .entry(cur_effect_field.clone())
                                .or_default()
                                .insert(pending_level, text.to_string());
                        }
                        _ => {}
                    }
                } else {
                    match path.len() {
                        // `<field>SCALAR</field>` directly under `<skill>`.
                        2 => {
                            values
                                .entry(cur_field.clone())
                                .or_default()
                                .insert(0, text.to_string());
                        }
                        // `<field><value fromLevel=… [fromSubLevel=…]>` — a
                        // ranged (possibly computed) row. Before this branch
                        // these rows fell into the level-0 slot below, where
                        // the last row's `{…}` text clobbered the field's
                        // scalar fallback.
                        3 if pending_range.is_some() => {
                            let mut row = pending_range.take().expect("checked");
                            row.text = text.to_string();
                            field_rows.entry(cur_field.clone()).or_default().push(row);
                        }
                        // `<field><value level="N">...`
                        3 => {
                            values
                                .entry(cur_field.clone())
                                .or_default()
                                .insert(pending_level, text.to_string());
                        }
                        _ => {}
                    }
                }
            }
            Event::End(_) => {
                let closed = path.pop().unwrap_or_default();
                if closed == "skill" {
                    finalize_skill(
                        skill_id,
                        &skill_name,
                        to_level,
                        &values,
                        &effects,
                        &field_rows,
                        &conditions,
                        out,
                    );
                    skill_id = -1;
                } else if closed.ends_with("ffects") {
                    in_effects = false;
                } else if closed.ends_with("onditions") {
                    cond_block = None;
                } else if closed == "condition" {
                    if let (Some(block), Some(name)) = (&cond_block, &cur_cond_name)
                        && let Some(scope) = CondScope::from_xml(block)
                    {
                        conditions.push(ParsedCondition {
                            scope,
                            name: name.clone(),
                            params: std::mem::take(&mut cur_cond_params),
                            sub_params: std::mem::take(&mut cur_cond_sub_params),
                            lists: std::mem::take(&mut cur_cond_lists),
                        });
                    }
                    cur_cond_name = None;
                } else if closed == "item" && in_effects && cur_effect_field == "items" {
                    // Closes a `RestorationRandom` group (the inner
                    // `<item id=".." count=".."/>` is self-closing, so this
                    // `End` only ever fires for the outer group tag).
                    cur_restoration_groups.push(RestorationGroup {
                        chance: cur_group_chance,
                        items: std::mem::take(&mut cur_group_items),
                    });
                } else if closed == "effect"
                    && in_effects
                    && let Some(name) = cur_effect_name.take()
                {
                    effects.push(ParsedEffect {
                        scope: cur_scope,
                        name,
                        params: cur_effect_params.clone(),
                        sub_params: std::mem::take(&mut cur_effect_sub_params),
                        mode: cur_effect_mode.clone(),
                        groups: std::mem::take(&mut cur_restoration_groups),
                        armor_condition: cur_effect_armor,
                        weapon_condition: cur_effect_weapon,
                        from_level: cur_effect_levels.0,
                        to_level: cur_effect_levels.1,
                        from_sub_level: cur_effect_levels.2,
                        to_sub_level: cur_effect_levels.3,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Boot-time report of everything the skill parser dropped (G34 §S0).
///
/// The parser is **fail-open**: an unrecognised `<effect name>` / `<condition>`
/// / `targetType` / … is silently ignored, so a skill can cast, animate, burn
/// MP and enter reuse while doing nothing. This report exists so that is not
/// rediscovered one player report at a time.
///
/// It reports *why* a gap is ignored rather than only how many there are, by
/// splitting each category against the skill trees:
///
/// * **Reachable** — the name sits on a skill some tree can actually put on a
///   character. That is real parity debt, so the line is a `warn!` naming the
///   exact skill ids: someone can hit it in game today.
/// * **Off-chronicle** — everything else, and it is the bulk. The dist's
///   `skills/*.xml` is shared with far later chronicles and carries Territory
///   War / Gracia / Freya content (`StatUp`, `SummonAgathion`, `ExpModify`, …)
///   that no Interlude tree references. Ignoring those is a *decision*, so the
///   line is an `info!` — porting them would grow the port without changing
///   anything a player can reach.
///
/// The split needs the skill trees, which is why this runs from
/// `GameData::load` once they are parsed rather than inside
/// `SkillData::load_from` (where the old version could only ever print raw
/// totals, and said so).
///
/// **This is a log line, not the gate.** The authority is
/// `coverage_census::datapack_skill_coverage_census`, which does the same
/// intersection against the raw XML — deliberately not through these loaders,
/// so it cannot measure the port against itself — and holds a named
/// `(skill_id, reason)` list rather than a count.
pub fn log_gaps(gaps: &SkillGaps, learnable: &BTreeSet<i32>) {
    /// Worst-first `Name(count)`, capped so one line stays readable.
    fn summarise(mut top: Vec<(&str, usize)>, cap: usize) -> String {
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let listed = top
            .iter()
            .take(cap)
            .map(|(name, n)| format!("{name}({n})"))
            .collect::<Vec<_>>()
            .join(", ");
        match top.len().saturating_sub(cap) {
            0 => listed,
            rest => format!("{listed}, +{rest} more"),
        }
    }

    for (label, map) in gaps.categories() {
        if map.is_empty() {
            continue;
        }

        // Split the names by whether any skill carrying them is learnable.
        let (reachable, off_chronicle): (Vec<_>, Vec<_>) = map
            .iter()
            .partition(|(_, ids)| ids.iter().any(|id| learnable.contains(id)));

        if !reachable.is_empty() {
            let named = reachable
                .iter()
                .map(|(name, ids)| {
                    let mut hit: Vec<i32> = ids
                        .iter()
                        .copied()
                        .filter(|id| learnable.contains(id))
                        .collect();
                    hit.sort_unstable();
                    let shown = hit.iter().map(i32::to_string).collect::<Vec<_>>().join("/");
                    format!("{name} (skill {shown})")
                })
                .collect::<Vec<_>>()
                .join("; ");
            warn!(
                "SkillData: <{label}> — {} name(s) unhandled on skills a player can actually \
                 learn, so those skills are wrong in game: {named}. Each of these should be a \
                 recorded decision in coverage_census::datapack_skill_coverage_census; one \
                 that is not on that list is an unrecorded gap.",
                reachable.len()
            );
        }

        if !off_chronicle.is_empty() {
            let skills: BTreeSet<i32> = off_chronicle
                .iter()
                .flat_map(|(_, ids)| ids.iter().copied())
                .collect();
            let top = off_chronicle
                .iter()
                .map(|(name, ids)| (name.as_str(), ids.len()))
                .collect();
            info!(
                "SkillData: <{label}> — {} unhandled name(s) across {} skill(s), none reachable \
                 from any skill tree. Ignored on purpose: the dist ships later chronicles' \
                 skill data (Territory War / Gracia / Freya), which parses here but is not \
                 Interlude content. {}",
                off_chronicle.len(),
                skills.len(),
                summarise(top, 12),
            );
        }
    }
}
