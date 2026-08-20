//! **The G34 coverage census** — the skill-parity gate, run against the real
//! dist datapack.
//!
//! `SkillGaps` (gameserver `data/skill_data`) records what the skill parser
//! dropped; this test intersects that with what the rest of the datapack can
//! actually *reach*, and asserts the result. The whole point is that the
//! numbers are checked in: land a handler and the census fails until its name
//! is struck off, so coverage can only move deliberately.
//!
//! **Reachability is measured from the raw XML, not from the ported loaders.**
//! A text scan answers "does the dist reference this skill id anywhere",
//! which is the honest denominator; going through `SkillTreeData`/`NpcData`
//! would measure the port's own coverage of *those* files instead and quietly
//! shrink the gap it is supposed to expose.
//!
//! Lives in the `tools` crate with the rest of the datapack tooling because it
//! is a measurement over the datapack, not server code — and like every other
//! tool here it links the server's own parser, so a verdict here is a verdict
//! in game.

use gameserver::data::skill_data::{GapMap, SkillData};
use std::collections::BTreeSet;

mod common;
use common::{DIST, ids_in, learnable, scan};

/// Everything the datapack can put in front of a player at all: learnable
/// + NPC/monster skill lists + item skills + pet skills.
fn reachable() -> BTreeSet<i32> {
    let mut out = learnable();
    scan("data/stats/npcs", false, &[r#"<skill id=""#], &mut out);
    scan(
        "data/stats/items",
        false,
        &[r#"<skill id=""#, r#"skillId=""#],
        &mut out,
    );
    let pets = std::fs::read_to_string(format!("{DIST}data/PetSkillData.xml")).unwrap_or_default();
    ids_in(&pets, r#"skillId=""#, &mut out);
    out
}

/// `name → how many of `scope`'s ids declared it`, worst first.
fn ranked(map: &GapMap, scope: &BTreeSet<i32>) -> Vec<(String, usize)> {
    let mut rows: Vec<(String, usize)> = map
        .iter()
        .map(|(name, ids)| (name.clone(), ids.intersection(scope).count()))
        .filter(|(_, n)| *n > 0)
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows
}

/// Skills in `scope` that lost *something* to this category.
fn affected(map: &GapMap, scope: &BTreeSet<i32>) -> BTreeSet<i32> {
    map.values()
        .flatten()
        .filter(|id| scope.contains(id))
        .copied()
        .collect()
}

/// `<effect>` names with at least one **learnable** skill behind them —
/// the work list, worst first. Category totals: 125 name(s), 9 learnable
/// skill(s) affected, 900 reachable — the live figures the tuple below
/// asserts. (This header had drifted to 143/10/1167, the totals from before
/// the S6 and S9 passes; it is a comment, so nothing failed when it went
/// stale.)
///
/// 216 → 214 at G34 S2: `PhysicalAbnormalResist`/`MagicalAbnormalResist`
/// joined `EFFECT_REGISTRY` once `Formulas.getAbnormalResist` had a
/// consumer. Neither has a learnable source, so the *learnable* count is
/// unchanged — which is the shape to expect from the item-only tail.
/// 214 → 205 at G34 S3: the nine flag-only effects, five of them with a
/// learnable source (`BuffBlock`, `PhysicalShieldAngleAll`, `Passive`,
/// `BlockResurrection`, `BlockEscape`).
/// 205 → 202 at G34 S4's first sub-slice: `Breath`, `WeightLimit`,
/// `WeightPenalty` — each one a `Stat` **plus a consumer**, since a
/// registry line on its own only makes the census shrink.
/// 202 → 199 at sub-slice 2: `PhysicalSkillPower`, `MagicalSkillPower`,
/// `PhysicalSkillCriticalDamage` (+ its defence twin), all wired into the
/// damage paths Java reads them from.
/// → 195 at sub-slice 3: the aggro family — `HateAttack` (auto-attacks
/// only, Java's `skill == null` branch), `TargetMe` and
/// `TargetMeProbability` (playables only, Java's `isPlayable()` guard).
/// → 190 at sub-slice 4: the mitigation/counter family — `AreaDamage`,
/// `TransferDamageToSummon`, `CounterPhysicalSkill`, `SkillEvasion`,
/// `SkillTurning`.
/// → 188 at sub-slice 5: `EnlargeAbnormalSlot` (the buff-slot cap) and
/// `DispelBySlotMyself` (which spares an `irreplacableBuff`).
/// → 185 at sub-slice 6: `SkillMastery` + `SkillMasteryRate` (the
/// cooldown-collapse proc) and `Lucky` (empty in Java; the buff's presence
/// is the mechanic).
/// → 182 at sub-slice 7: `CriticalRatePositionBonus`, `MpVampiricAttack`
/// and `CubicMastery` (whose `MAX_CUBIC` nothing in Java reads).
/// → 178 at sub-slice 8: the heal-ceiling family — `LimitHp`/`LimitCp`
/// (`MAX_RECOVERABLE_*`, which every heal clamp now honours),
/// `CpHealPercent` and `HpByLevel`.
/// → 175 at sub-slice 9: `Bluff` (the Backstab set-up spin, with Java's
/// raid exemption), `Unsummon` and `DeathLink` (power scaled by the
/// caster's *missing* HP — it does nothing at full health).
/// → 173 at sub-slice 10: `OpenDoor` and `OpenChest`, i.e. the whole of
/// Unlock (27) — which also needed the `DOOR_TREASURE` target type, so
/// the target axis loses its last learnable entry too.
/// → 170 at sub-slice 11: the sustain family — `ChameleonRest` (Relax
/// without its HP-full stop, plus `SILENT_MOVE`), `ManaHealOverTime` (the
/// mirror of the MP drain) and `RebalanceHP` (Balance Life, which is a
/// redistribution rather than a heal).
/// → 155 at sub-slice 12: the whole PvP/PvE balance family — fifteen names
/// in one go, because they share one consumer
/// (`formulas::calculate_pvp_pve_bonus`) that had to be written first. The
/// biggest single drop in *reachable* terms this epic has seen: 1684 →
/// 1355.
/// → 153 at sub-slice 13: the physical-attack pair —
/// `PhysicalAttackHpLink` (Fatal Counter's missing-HP scaling, sharing
/// `PhysicalAttack`'s arm) and `PolearmSingleTarget` (Focus Attack's
/// *cost*, which had been missing while both its bonuses landed).
/// → 151 at sub-slice 14: the trigger pair — `TriggerSkillByDamage`
/// (Mirage, fired on damage *received*) and `TriggerSkillByMagicType`
/// (Dance of Shadows, fired when the bearer finishes a cast).
/// → 148 at sub-slice 15: servitors and recall — `Betray` (the servitor
/// turns on its owner, stops obeying and becomes auto-attackable),
/// `ImmobilePetBuff` and `CallParty` (Chant of Gate, a recall with no
/// `ConfirmDlg`).
/// → 146 at sub-slice 16: the death pair — `ReduceDropPenalty` (the
/// exp-loss reduction, keyed on what killed you) and `ResurrectionSpecial`
/// (the auto-resurrect, which fires from `onExit`).
/// → 145 at sub-slice 17: `NightStatModify` (Shadow Sense 294 — the stat
/// belongs to the *clock*, not the buff). **This closes S4**: the two
/// names left with a learnable source, `StatUp` (Territory War) and
/// `SafeFallHeight` (needs fall damage), are both deliberately out of
/// scope, so the residue is now entirely recorded rather than unexamined.
/// → 144 with the `General.ini` fall-damage cluster: `SafeFallHeight` had
/// been parked on "needs fall damage", and fall damage now exists
/// (`game_loop::falling`), so the effect is registered against `Stat::Fall`
/// and Acrobatics (173) works. Reachable 975 → 974. `StatUp` (Territory
/// War) is the only learnable-source name left, and that one is
/// off-chronicle for good.
/// → 143 at S6: the item tail's two biggest names — `Teleport` (every
/// destination Scroll of Escape, 107 reachable, all previously inert) and
/// `Hp` (Elixir of Life and the food items). Reachable 1303 → 1167.
/// → 134 at **S9**, the live-reachability pass. S4 onward ranked the tail by
/// *reachable*, which counts any skill the datapack merely mentions — and
/// most of what was left is later-chronicle content this server can never
/// hand a player (Agathion, brooch jewels, Sayha's, Vitality runes).
/// Re-ranking by what is actually **obtainable** — an NPC that appears in a
/// spawn file, an item off one of their drop lists, a buylist, a multisell, a
/// recipe or a quest reward — cut 1154 reachable to 611 live, and left a
/// short list of real Interlude content standing out of it. These four are
/// that list's effect half: `DispelAll` (skill 4177 Cancellation, cast by
/// ~40 raid bosses and until now a no-op), `GiveSp` (SP Scrolls and the
/// Primeval Isle crystals), `TeleportToTarget` (the Splendor mobs'
/// gap-closer), `SetSkill` (the Ancient Book: Divine Inspiration family) and
/// `Grow` (the collision swell on Might/Ultimate Buff/Berserker Spirit, which
/// the Orc Prefect and Grandis families cast — 1891 templates on this dist
/// declare the `grown` cylinder it reads). Reachable 1154 → 975.
///
/// → 126 at **row 18**, the live tail's own pass: the appearance potions
/// (`ChangeFace`, `ChangeHairStyle`, `ChangeHairColor` — Facelifting, Hair
/// Style Change and Dye, all ordinary Interlude items), `SendSystemMessageToClan`
/// (Clan Gate), `Recovery` (**empty in Java** — the whole body of its
/// `instant()` is commented out, so registering it is the honest answer) and
/// `VitalityPointsRate`. That last one overturned a note in the port:
/// `game_loop::vitality` said no skill on this dist granted
/// `VITALITY_CONSUME_RATE`, and skill 2580 does — its carrier herbs drop from
/// the Schuttgart golems this dist spawns. Reachable 974 → 900.
///
/// → 125 at the **reachable-tail re-derivation**, which re-ran row 18's
/// classification instead of trusting it. Of the 175 effect names with no
/// learnable carrier, **15** are live once "reachable" is tightened to *a
/// spawned NPC, a drop off one, or a shop bound to one* — and 14 of the 15 were
/// already handled. The fifteenth was `VampiricDefence`: skill 14765 "Blood
/// Siphon Resistance" sits on 891 templates, four of them spawned (Queen
/// Shyeed, Plague Golem, Flamestone Giant, Uruka); reachable 900 → 899. The other four names that
/// looked live under a looser rule — `ExpModify`, `SpModify`, `DamageByAttack`,
/// `ChangeFishingMastery` — are not: their carrier items sit in a multisell
/// with no NPC binding and two buylists whose NPCs (35274, 36657) are never
/// spawned.
///
/// `Escape` is **not** off this list, and should not be: its TOWN, CLANHALL
/// and CASTLE arms are all ported now — which is what un-inerted the Scrolls
/// of Escape: Clan Hall/Castle and their blessed twins — but `FORTRESS` still
/// drops on purpose. There is no fortress system to teleport to, and letting
/// it fall back to town would hide that behind a plausible-looking result.
const EFFECTS: &[(&str, usize)] = &[("StatUp", 9)];

/// `<effect-scope>` names with at least one **learnable** skill behind them —
/// the work list, worst first. Category totals: 5 name(s), 1 learnable
/// skill(s) affected, 10 reachable.
const EFFECT_SCOPES: &[(&str, usize)] = &[];

/// `<condition>` names with at least one **learnable** skill behind them —
/// the work list, worst first. Category totals: 55 name(s), 1 learnable
/// skill(s) affected, 716 reachable.
///
/// **G34 S1 took this from 111 names / 215 learnable skills down to this.**
/// The single learnable hold-out is `OpSweeper`, left out on purpose: Java's
/// version re-runs the skill's whole affect-scope sweep and asks each corpse
/// about spoil ownership, corpse age and the sweeper's free inventory — all
/// of which `effects::sweep` already does at *apply* time with the right
/// per-corpse messages. Gating the cast on it too would double every one.
///
/// 60 → 55 names at row 18: `CanSummonPet` (the collar gate, a different
/// chain from `CanSummon`'s servitor one), `OpMainjob`, `CannotUseInTransform`,
/// `OpPledge` and `OpCheckResidence`. Reachable 733 → 716.
///
/// 69 → 61 names at S9, the live-reachability pass — six Java handlers, eight
/// names, because `OpTargetNpc` and `OpAlignment` each appear under two
/// scopes. `OpHome` (the blessed scrolls' residence gate), `OpTargetDoor`
/// (the Four Sepulchers keys), `OpTargetNpc` (Nectar on a squash),
/// `OpCompanion` (Blessed Scroll of Resurrection (Pet)), `OpAlignment` (the
/// race-village scrolls) and `OpSkill` (the Ancient Books). Reachable
/// 916 → 734.
const CONDITIONS: &[(&str, usize)] = &[("conditions/OpSweeper", 1)];

/// `<targetType>` names with at least one **learnable** skill behind them —
/// the work list, worst first. Category totals: 8 name(s), 0 learnable
/// skill(s) affected, 457 reachable.
///
/// 9 → 8 at S9: `OWNER_PET`. It had been folded into `TargetType::Other`
/// with a note claiming every reachable carrier resolved correctly anyway —
/// which was true of the tamed-beast buffs and false of Master Recharge
/// (4025), the skill every Baby Kookaburra carries. See the `EFFECTS` note.
const TARGET_TYPES: &[(&str, usize)] = &[];

/// `<affectScope>` names with at least one **learnable** skill behind them —
/// the work list, worst first. Category totals: 7 name(s), 0 learnable
/// skill(s) affected, 3 reachable.
const AFFECT_SCOPES: &[(&str, usize)] = &[];

/// `<affectObject>` names with at least one **learnable** skill behind them —
/// the work list, worst first. Category totals: 4 name(s), 0 learnable
/// skill(s) affected, 2 reachable.
const AFFECT_OBJECTS: &[(&str, usize)] = &[];

/// `<operateType>` names with at least one **learnable** skill behind them —
/// the work list, worst first. Category totals: 7 name(s), 0 learnable
/// skill(s) affected, 27 reachable.
const OPERATE_TYPES: &[(&str, usize)] = &[];

/// **The gate.** Every entry above is a skill that loads and then behaves
/// wrongly — an effect that does nothing, or a condition Java would have
/// refused the cast on. Landing a slice makes this fail; strike the ported
/// names off the tables and move the totals down, never up.
///
/// A failing count is telling you *what changed*, not *what number to
/// write*: read which names appeared or vanished before touching a
/// constant. Adding a datapack file can enable unrelated content in it.
#[test]
fn datapack_skill_coverage_census() {
    let sd = SkillData::load_from(DIST);
    let gaps = sd.gaps();
    let learn = learnable();
    let reach = reachable();

    // The denominators first — every number below is a fraction of these,
    // so a change here explains changes everywhere else.
    assert_eq!(
        learn.len(),
        758,
        "learnable skill ids in data/skillTrees/** — the ranking denominator"
    );
    assert_eq!(
        reach.len(),
        10704,
        "skill ids the datapack references at all (learnable + npc + item + pet)"
    );

    for (label, map, expected, names, learn_hit, reach_hit) in [
        // `CallPc` is now handled on **both** halves — the monster drag
        // (Porta 20213 / skill 4161) and the player prompt (Summon Friend
        // 1403 and its siblings) — so counting it as handled no longer
        // overstates anything. It did until 2026-08-06.
        ("effect", &gaps.effects, EFFECTS, 125, 9, 899),
        ("effect-scope", &gaps.effect_scopes, EFFECT_SCOPES, 2, 0, 1),
        ("condition", &gaps.conditions, CONDITIONS, 55, 1, 716),
        ("targetType", &gaps.target_types, TARGET_TYPES, 8, 0, 457),
        ("affectScope", &gaps.affect_scopes, AFFECT_SCOPES, 7, 0, 3),
        (
            "affectObject",
            &gaps.affect_objects,
            AFFECT_OBJECTS,
            4,
            0,
            2,
        ),
        ("operateType", &gaps.operate_types, OPERATE_TYPES, 7, 0, 27),
    ] {
        assert_eq!(
            ranked(map, &learn)
                .iter()
                .map(|(n, c)| (n.as_str(), *c))
                .collect::<Vec<_>>(),
            expected.to_vec(),
            "<{label}>: unhandled names with a learnable source (PLAN_G34_SKILL_PARITY.md)"
        );
        assert_eq!(map.len(), names, "<{label}>: total unhandled names");
        assert_eq!(
            affected(map, &learn).len(),
            learn_hit,
            "<{label}>: learnable skills that lose something to it"
        );
        assert_eq!(
            affected(map, &reach).len(),
            reach_hit,
            "<{label}>: reachable skills that lose something to it"
        );
    }

    // The epic's headline number, and its gate: a learnable skill is
    // "wrong" if it drops an effect (silently does nothing) or ignores a
    // condition (fires where Java refuses). G34 closes when this is 0 bar
    // an explicitly recorded out-of-chronicle list.
    let mut wrong: BTreeSet<i32> = BTreeSet::new();
    wrong.extend(affected(&gaps.effects, &learn));
    wrong.extend(affected(&gaps.effect_scopes, &learn));
    wrong.extend(affected(&gaps.conditions, &learn));
    //
    // **G34's close-out gate (S8).** Counting the residue is not enough —
    // a shrinking number says nothing about whether what is left was
    // *decided* or merely never looked at. So the gate names every
    // remaining skill and the reason it is out of scope. A new gap cannot
    // hide inside the count; it shows up as an id with no entry here.
    let out_of_scope: &[(i32, &str)] = &[
        // `StatUp` — the nine "<Town> Territory Benefaction" skills, i.e.
        // Territory War content, which this chronicle has none of.
        (848, "StatUp: Gludio Territory Benefaction (Territory War)"),
        (849, "StatUp: Dion Territory Benefaction (Territory War)"),
        (850, "StatUp: Giran Territory Benefaction (Territory War)"),
        (851, "StatUp: Oren Territory Benefaction (Territory War)"),
        (852, "StatUp: Aden Territory Benefaction (Territory War)"),
        (
            853,
            "StatUp: Innadril Territory Benefaction (Territory War)",
        ),
        (854, "StatUp: Goddard Territory Benefaction (Territory War)"),
        (855, "StatUp: Rune Territory Benefaction (Territory War)"),
        (
            856,
            "StatUp: Schuttgart Territory Benefaction (Territory War)",
        ),
        // `OpSweeper` — Sweeper (42), deliberate: see `CONDITIONS` above.
        // Java's cast condition re-runs the whole per-corpse sweep that
        // `effects::sweep` already does at apply time, with the right
        // per-corpse messages. Gating the cast on it too would double
        // every one of them.
        (42, "OpSweeper: Sweeper — enforced at apply time instead"),
    ];
    let recorded: BTreeSet<i32> = out_of_scope.iter().map(|(id, _)| *id).collect();
    let unexplained: Vec<i32> = wrong.difference(&recorded).copied().collect();
    assert!(
        unexplained.is_empty(),
        "G34's gate: every learnable skill still carrying an unhandled effect or an \
         unenforced condition must be on the recorded out-of-scope list with a \
         reason. These are not: {unexplained:?} (of {} learnable; was 275 before \
         G34 S1)",
        learn.len()
    );
    // And the list must not rot the other way: an id that has since been
    // ported should come *off* the list rather than sit there excusing a
    // gap that no longer exists.
    let stale: Vec<i32> = recorded.difference(&wrong).copied().collect();
    assert!(
        stale.is_empty(),
        "recorded as out of scope but no longer failing — delete these entries: \
         {stale:?}"
    );
}

/// The `TODO(<tag>)` deferral inventory — **currently empty**.
///
/// Every milestone row in `PROGRESS.md` is ✅, and each one still shipped a
/// handful of deliberately-deferred behaviours marked at the site. That is the
/// intended end state, not a contradiction — but it is invisible from the
/// status table, and prose about "what is left" is the least reliable artefact
/// in this repo: on 2026-08-03 two separate documents were found claiming work
/// was outstanding that had been done for milestones, and five `TODO` markers
/// PROGRESS said existed were absent from the code entirely.
///
/// So the count is asserted rather than described. It moves only deliberately:
/// adding a gap without recording it fails, and closing one without updating
/// the number fails too — the same two-way discipline G34's close-out gate
/// uses.
///
/// **This counts every `TODO(<tag>)` marker, not just the milestone ones.** It
/// did not always: the scan looked for `TODO(G` and accepted only
/// alphanumerics, `.`, `-` and `_` in the tag, so it silently dropped nine
/// milestone markers written with a `+`, `?` or `/` suffix, and never saw the
/// topic-tagged family at all. An inventory blind to 46 of its own subjects
/// reports a number that only looks trustworthy, which is the exact failure
/// this test exists to prevent — so the scan is now total and the allowlist,
/// not the prefix, is what distinguishes a marker from prose about one.
#[test]
fn deferral_markers_match_the_recorded_inventory() {
    use std::collections::BTreeMap;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .to_path_buf();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                // Skip build output — `target/` mirrors sources into deps.
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for (i, _) in text.match_indices("TODO(") {
                    let rest = &text[i + "TODO(".len()..];
                    if let Some(end) = rest.find(')') {
                        let tag = &rest[..end];
                        // The allowlist is what separates a marker from prose
                        // that merely mentions one. `+ ? /` are in it because
                        // real markers use them ("G9+" = this milestone and
                        // later, "G24/G26" = split across two); `<` `"` and
                        // whitespace are out, which is what keeps this file's
                        // own scanner literal and its `TODO(G<N>)` doc prose
                        // from counting themselves. Never write a parseable
                        // tag in prose — it will be counted.
                        if !tag.is_empty()
                            && tag.chars().all(|c| {
                                c.is_ascii_alphanumeric()
                                    || matches!(c, '.' | '-' | '_' | '+' | '?' | '/')
                            })
                        {
                            *counts.entry(tag.to_owned()).or_default() += 1;
                        }
                    }
                }
            }
        }
    }

    // Byte-sorted, as `BTreeMap` yields them. This list *is* the inventory:
    // `docs/DEFERRALS.md` was its human-readable half until 2026-08-07, when
    // the last marker closed and the file was deleted (recoverable from git —
    // see `docs/PORTING_STATUS.md`). A gap recorded here should also be
    // described wherever its milestone is written up, usually PROGRESS.md.
    //
    // Two families live here. Milestone tags (`G<N>`) are deferrals recorded
    // against a shipped milestone's gate. Topic tags (lowercase) are the ones
    // that never had a milestone to hang off — they are gaps just the same,
    // and were invisible to this test until the scan widened past `TODO(G`.
    // 2026-08-08, the bare-marker triage: the inventory is **no longer empty**,
    // and that is a correction rather than a regression. These six gaps were all
    // sitting in the tree as untagged prose — invisible here by construction —
    // and each was confirmed against the datapack before being written down.
    // `armor-sets` was the one that mattered and is now closed — `ArmorSetData`
    // is ported (`data::armor_set_data` + `game_loop::armor_sets`).
    // Every gap the bare-marker triage recorded has since been closed, so the
    // only entry left is one deduplication turned up. A new `TODO(<tag>)` fails
    // this test until it is listed.
    //
    // `antharas-cc` was found by deduplication, not by a sweep: the entry gate's
    // doc claimed the command channel outranked the party, while the body it
    // documented only ever read `PartyRef` — byte-identical to sailren's
    // honestly-party-only twin. The claim was prose, so nothing here saw it.
    let expected: &[(&str, usize)] = &[("antharas-cc", 1)];
    let actual: Vec<(String, usize)> = counts.into_iter().collect();
    let expected: Vec<(String, usize)> = expected
        .iter()
        .map(|(t, n)| ((*t).to_owned(), *n))
        .collect();
    assert_eq!(
        actual, expected,
        "the TODO(G<N>) deferral inventory moved. If you recorded a new gap, add it \
         here and describe it where its milestone is written up; if you closed \
         one, take it off. A gap that is not in this list is invisible to \
         everyone who reads PROGRESS.md."
    );
}

#[test]
#[ignore = "reporting aid, not an assertion — run with --ignored --nocapture"]
fn print_coverage_report() {
    let sd = SkillData::load_from(DIST);
    let gaps = sd.gaps();
    let learn = learnable();
    let reach = reachable();
    println!("learnable={} reachable={}", learn.len(), reach.len());
    let mut union: BTreeSet<i32> = BTreeSet::new();
    union.extend(affected(&gaps.effects, &learn));
    union.extend(affected(&gaps.effect_scopes, &learn));
    union.extend(affected(&gaps.conditions, &learn));
    println!(
        "union(effects|scopes|conditions) learnable-affected = {}",
        union.len()
    );
    for (label, map) in gaps.categories() {
        println!(
            "\n// {label}: {} names, {} learnable-affected, {} reachable-affected",
            map.len(),
            affected(map, &learn).len(),
            affected(map, &reach).len(),
        );
        for (name, n) in ranked(map, &learn) {
            println!("    (\"{name}\", {n}),");
        }
        // The **reachable** ranking too: past S4 the learnable list is
        // essentially empty, and the remaining work (S6's item/NPC tail) is
        // ranked by what item- and NPC-triggered skills lose, which the
        // learnable ranking cannot see at all.
        let by_reach = ranked(map, &reach);
        if !by_reach.is_empty() {
            println!("    // by reachable:");
            for (name, n) in by_reach.iter().take(25) {
                println!("    //   (\"{name}\", {n}),");
            }
        }
    }
}
