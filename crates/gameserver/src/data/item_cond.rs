//! `<cond>` on `stats/items/*.xml` — Java `DocumentBase.parseCondition` and the
//! `ItemTemplate._preConditions` list `DocumentItem` fills from it.
//!
//! This is the **legacy** condition tree, and it is not the one skills use.
//! `SkillData` reads named `<condition name="OpXxx">` elements
//! ([`crate::model::skill::condition::SkillCondition`]); `<cond>` is an attribute-driven
//! tree of `<and>`/`<or>`/`<not>` over `<player …>`/`<target …>` elements,
//! parsed by `DocumentBase` — whose **only** subclass in the Java tree is
//! `DocumentItem`. That matters twice over:
//!
//! * 13 files under `stats/skills` also carry `<cond>` blocks. Java never reads
//!   them: `SkillData` is not a `DocumentBase`, so those blocks are inert data
//!   on both sides. Only the item ones are ported here.
//! * An item may carry **several** `<cond>` blocks. `attachCondition` appends
//!   each to a list that `checkCondition` walks in order, so they are ANDed —
//!   but each carries its **own** refusal message, which is why the list is not
//!   collapsed into one [`Cond::And`].
//!
//! # Java's null is this module's `None`, and it fails open
//!
//! `parsePlayerCondition` switches on the attribute name with **no default
//! arm**: an attribute it does not know contributes nothing and the others
//! still build. Only when *no* attribute matched does the element parse to
//! `null` — logged `severe` and attached as a null pre-condition, which
//! `checkCondition` skips with a `continue`. Reproduced exactly: an unmatched
//! attribute is dropped, an element with nothing left is [`None`], and the
//! evaluator skips a `<cond>` block that parsed to nothing.
//!
//! Two of Java's attributes are parsed here into conditions that are
//! **constant** on this dist rather than left unported, because that is what
//! Java computes, not a narrowing:
//!
//! * `fort="-1"` (one item, 10018) — `ConditionPlayerHasFort` asks the clan for
//!   `getFortId()`. There are no fortresses on this chronicle, so the answer is
//!   0 for every clan and the condition is false for everyone. [`Cond::HasFort`]
//!   reads a clan fort id the port models as always 0.
//! * `cloakStatus="true"` (49 items, all ≥ 13686) — `Inventory.canEquipCloak()`
//!   forwards to `PlayerStat._cloakSlot`, whose only setter
//!   (`setCloakSlotStatus`) has **no caller anywhere in the Java tree**. The
//!   flag is therefore false for every character in Java too.
//!
//! See [`crate::game_loop::items::conditions`] for the evaluator.

use serde::{Deserialize, Serialize};

use crate::enums::Race;
use crate::model::castle::CastleSide;

/// One `<cond>` block: the tree, plus the message its failure sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemCondition {
    pub node: Cond,
    pub message: CondMessage,
}

/// `Condition.setMessage` / `setMessageId` + `addName` — what a failing block
/// tells the player. Java sets **either** the string or the id (`msg` wins; the
/// `addName` flag only exists on the id branch, and only when the id is > 0).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum CondMessage {
    /// No `msg`/`msgId` on the block: the refusal is silent.
    #[default]
    Silent,
    /// `msg="…"` — Java `creature.sendMessage(msg)`.
    Text(String),
    /// `msgId="…"` — a `SystemMessage`, carrying the item name when
    /// `addName="1"`.
    Sm { id: i16, add_name: bool },
}

/// A node of the `<cond>` tree. The leaves are named for the Java `Condition`
/// class each one is (`ConditionPlayerRace` → [`Cond::Race`]).
///
/// A `<player>` element with several attributes becomes an [`Cond::And`] over
/// its leaves — Java's `joinAnd` chain, which builds exactly that.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Cond {
    /// `<and>` — `ConditionLogicAnd`.
    And(Vec<Cond>),
    /// `<or>` — `ConditionLogicOr`.
    Or(Vec<Cond>),
    /// `<not>` — `ConditionLogicNot`.
    Not(Box<Cond>),

    // ---- `<player …>` ----
    /// `races="HUMAN,ELF"` — `ConditionPlayerRace`. False for a non-player
    /// effector (a pet cannot pass a race gate).
    Race(Vec<Race>),
    /// `level="80"` — `ConditionPlayerLevel`, a **minimum** (`>=`), and it
    /// reads the *effector's* level, so a pet's own level answers it.
    Level(i32),
    /// `levelRange="76;84"` — `ConditionPlayerLevelRange`, inclusive.
    LevelRange(i32, i32),
    /// `chaotic="false"` — `ConditionPlayerState(CHAOTIC)`: reputation < 0.
    Chaotic(bool),
    /// `isHero="true"` — `ConditionPlayerIsHero`.
    IsHero(bool),
    /// `pkCount="0"` — `ConditionPlayerPkCount`, an inclusive **maximum**
    /// (`getPkKills() <= value`), the reverse of what the name suggests.
    PkCount(i32),
    /// `SiegeZone="126"` — `ConditionSiegeZone`. `self_` is false only for the
    /// `<target SiegeZone>` spelling, which no item uses.
    SiegeZone { value: i32, self_: bool },
    /// `isClanLeader="true"` — `ConditionPlayerIsClanLeader`.
    IsClanLeader(bool),
    /// `pledgeClass="4"` — `ConditionPlayerPledgeClass`: a minimum, except that
    /// a clan **leader** passes any value and `-1` means leaders only.
    PledgeClass(i32),
    /// `clanHall="36, 37"` — `ConditionPlayerHasClanHall`. A lone `-1` means
    /// "any hall"; a lone `0` is the clanless case.
    HasClanHall(Vec<i32>),
    /// `fort="-1"` — `ConditionPlayerHasFort`; see the module header.
    HasFort(i32),
    /// `castle="-1"` — `ConditionPlayerHasCastle`. `-1` = any castle.
    HasCastle(i32),
    /// `sex="1"` — `ConditionPlayerSex`, `1` = female.
    Sex(i32),
    /// `flyMounted="false"` — `ConditionPlayerFlyMounted`. **True** when the
    /// effector has no acting player at all.
    FlyMounted(bool),
    /// `vehicleMounted="false"` — `ConditionPlayerVehicleMounted`, likewise
    /// true with no acting player.
    VehicleMounted(bool),
    /// `class_id_restriction="188"` — `ConditionPlayerClassIdRestriction`, an
    /// allow-list of class ids.
    ClassIdRestriction(Vec<i32>),
    /// `subclass="false"` — `ConditionPlayerSubclass`: is a subclass active.
    Subclass(bool),
    /// `instanceId="43,44"` — `ConditionPlayerInstanceId`, matched against the
    /// instance **template** id.
    InstanceId(Vec<i32>),
    /// `cloakStatus="true"` — `ConditionPlayerCloakStatus`; see the module
    /// header for why it is constant.
    CloakStatus(bool),
    /// `insideZoneId="12010, 12001"` — `ConditionPlayerInsideZoneId`.
    InsideZoneId(Vec<i32>),
    /// `categoryType="STRIDER"` — `ConditionCategoryType`, resolved against
    /// `CategoryData.xml` with the **effector's** id: a player's class id, a
    /// pet's npc id.
    CategoryType(Vec<String>),
    /// `isOnSide="LIGHT"` — `ConditionPlayerIsOnSide`.
    IsOnSide(CastleSide),
    /// `MinimumVitalityPoints="35000"` — `ConditionMinimumVitalityPoints`.
    MinimumVitalityPoints(i32),

    // ---- `<target …>` ----
    /// `<target levelRange="…">` — `ConditionTargetLevelRange`. One item on
    /// this dist uses it, and `checkCondition` passes the effector as the
    /// target, so it asks the same question as [`Cond::LevelRange`] on the
    /// `UseItem` path.
    TargetLevelRange(i32, i32),
}

/// `DocumentItem`'s `<cond>` arm: the block's message attributes.
///
/// `msg` wins over `msgId` (Java's `else if`), and `addName` is read only on
/// the id branch and only when the id is positive.
pub(crate) fn message_from(
    msg: Option<String>,
    msg_id: Option<&str>,
    add_name: Option<&str>,
) -> CondMessage {
    if let Some(text) = msg {
        return CondMessage::Text(text);
    }
    let Some(id) = msg_id.and_then(decode_i32) else {
        return CondMessage::Silent;
    };
    CondMessage::Sm {
        id: id as i16,
        add_name: add_name.is_some() && id > 0,
    }
}

/// `parsePlayerCondition` — the attribute switch, `joinAnd`ed.
///
/// `None` is Java's `cond == null`: nothing on the element was recognised,
/// which it logs as `severe` and attaches as a null pre-condition.
pub(crate) fn player_condition(attrs: &[(String, String)]) -> Option<Cond> {
    let mut out: Vec<Cond> = Vec::new();
    for (name, value) in attrs {
        // Java lowercases the attribute name before the switch, so
        // `SiegeZone` and `MinimumVitalityPoints` reach lower-case arms.
        //
        // Every arm yields an `Option` because half of them read a number:
        // `None` is a value `Integer.decode` would have thrown on. Neither
        // tree has one — every value on this dist parses — so the choice
        // between Java's "lose the file" and dropping the attribute is
        // unobservable, and dropping keeps the rest of the item.
        let leaf = match name.to_ascii_lowercase().as_str() {
            "races" => Some(Cond::Race(
                value
                    .split(',')
                    .filter_map(|r| Race::from_name(r.trim()))
                    .collect(),
            )),
            "level" => decode_i32(value).map(Cond::Level),
            "levelrange" => level_range(value).map(|(lo, hi)| Cond::LevelRange(lo, hi)),
            "chaotic" => Some(Cond::Chaotic(parse_bool(value))),
            "ishero" => Some(Cond::IsHero(parse_bool(value))),
            "pkcount" => decode_i32(value).map(Cond::PkCount),
            "siegezone" => decode_i32(value).map(|value| Cond::SiegeZone { value, self_: true }),
            "isclanleader" => Some(Cond::IsClanLeader(parse_bool(value))),
            "pledgeclass" => decode_i32(value).map(Cond::PledgeClass),
            "clanhall" => Some(Cond::HasClanHall(int_list(value))),
            "fort" => decode_i32(value).map(Cond::HasFort),
            "castle" => decode_i32(value).map(Cond::HasCastle),
            "sex" => decode_i32(value).map(Cond::Sex),
            "flymounted" => Some(Cond::FlyMounted(parse_bool(value))),
            "vehiclemounted" => Some(Cond::VehicleMounted(parse_bool(value))),
            "class_id_restriction" => Some(Cond::ClassIdRestriction(int_list(value))),
            "subclass" => Some(Cond::Subclass(parse_bool(value))),
            "instanceid" => Some(Cond::InstanceId(int_list(value))),
            "cloakstatus" => Some(Cond::CloakStatus(parse_bool(value))),
            "insidezoneid" => Some(Cond::InsideZoneId(int_list(value))),
            "categorytype" => Some(Cond::CategoryType(
                value
                    .split(',')
                    .map(|c| c.trim().to_string())
                    .filter(|c| !c.is_empty())
                    .collect(),
            )),
            "isonside" => CastleSide::from_string(value).map(Cond::IsOnSide),
            "minimumvitalitypoints" => decode_i32(value).map(Cond::MinimumVitalityPoints),
            // No default arm in Java either: an unknown attribute contributes
            // nothing and the recognised ones still build.
            _ => continue,
        };
        if let Some(leaf) = leaf {
            out.push(leaf);
        }
    }
    join_and(out)
}

/// `parseTargetCondition`, narrowed to the attributes items use. The one
/// `<target>` element on this dist carries `levelRange`; `SiegeZone` is the
/// only other arm reachable from item data and shares [`Cond::SiegeZone`] with
/// `self_ = false`.
pub(crate) fn target_condition(attrs: &[(String, String)]) -> Option<Cond> {
    let mut out: Vec<Cond> = Vec::new();
    for (name, value) in attrs {
        let leaf = match name.to_ascii_lowercase().as_str() {
            "levelrange" => level_range(value).map(|(lo, hi)| Cond::TargetLevelRange(lo, hi)),
            "siegezone" => decode_i32(value).map(|value| Cond::SiegeZone {
                value,
                self_: false,
            }),
            _ => continue,
        };
        if let Some(leaf) = leaf {
            out.push(leaf);
        }
    }
    join_and(out)
}

/// `DocumentBase.joinAnd` — one condition stays bare, several become an
/// `ConditionLogicAnd`, none is `null`.
fn join_and(mut conds: Vec<Cond>) -> Option<Cond> {
    match conds.len() {
        0 => None,
        1 => conds.pop(),
        _ => Some(Cond::And(conds)),
    }
}

/// `levelRange="76;84"`. Java builds the condition only when the split gives
/// exactly two parts, and otherwise leaves `cond` untouched — so a malformed
/// value is the same as an absent attribute, not a parse failure.
fn level_range(value: &str) -> Option<(i32, i32)> {
    let parts: Vec<&str> = value.split(';').collect();
    if parts.len() != 2 {
        // Java's `if (range.length == 2)` falls through: nothing is joined.
        return None;
    }
    Some((decode_i32(parts[0])?, decode_i32(parts[1])?))
}

/// `Integer.decode` on a trimmed token. Every value on this dist is plain
/// decimal (possibly negative); the hex/octal prefixes `decode` also accepts
/// appear nowhere, so this is the decimal reader with the same failure mode —
/// Java would throw and lose the file, the port drops the attribute.
fn decode_i32(value: &str) -> Option<i32> {
    value.trim().parse().ok()
}

/// The `StringTokenizer(value, ",")` + `Integer.decode` pairs — `clanHall`,
/// `castle`'s list-free sibling aside, `class_id_restriction`, `instanceId`,
/// `insideZoneId`.
fn int_list(value: &str) -> Vec<i32> {
    value.split(',').filter_map(decode_i32).collect()
}

/// `Boolean.parseBoolean` — case-insensitive `"true"`, everything else false.
fn parse_bool(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("true")
}

impl Cond {
    /// The leaves of this tree, for tests and the coverage census.
    #[cfg(test)]
    pub(crate) fn leaves(&self) -> Vec<&Cond> {
        match self {
            Cond::And(cs) | Cond::Or(cs) => cs.iter().flat_map(Cond::leaves).collect(),
            Cond::Not(c) => c.leaves(),
            leaf => vec![leaf],
        }
    }
}

/// The `<cond>` sub-tree assembler `ItemData`'s event loop drives.
///
/// `DocumentBase` walks a DOM and can ask a node for its first element child;
/// the port streams events, so the same shape is kept on a stack. One
/// [`CondBuilder`] lives across the whole file and is idle (`depth == 0`)
/// everywhere outside a `<cond>` block.
///
/// Two of Java's DOM reads are worth spelling out, because they are *not*
/// "collect the children":
///
/// * `parseCondition(n.getFirstChild())` — a `<cond>` block, and a `<not>`,
///   take their **first element child and nothing else**. A second sibling is
///   silently dropped, on both sides.
/// * `parseLogicAnd`/`Or` do collect every element child.
#[derive(Debug, Default)]
pub(crate) struct CondBuilder {
    stack: Vec<Frame>,
    message: CondMessage,
}

#[derive(Debug)]
struct Frame {
    kind: Group,
    children: Vec<Cond>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Group {
    /// The `<cond>` element itself: keeps its first child.
    Root,
    And,
    Or,
    /// `<not>`: keeps its first child, negated.
    Not,
}

impl CondBuilder {
    /// `<cond …>` — start a block. The message attributes are read here
    /// because Java reads them off the `<cond>` node, not off the condition.
    pub(crate) fn begin(&mut self, message: CondMessage) {
        self.stack.clear();
        self.message = message;
        self.stack.push(Frame {
            kind: Group::Root,
            children: Vec::new(),
        });
    }

    /// Are we inside a `<cond>` block? The loop uses this to route `<player>`
    /// and the logic elements, which are otherwise ordinary element names.
    pub(crate) fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    /// `<and>` / `<or>` / `<not>` — open a group. Anything else is ignored,
    /// which is `parseCondition`'s switch with no default.
    pub(crate) fn open_group(&mut self, name: &[u8]) {
        let kind = match name {
            b"and" => Group::And,
            b"or" => Group::Or,
            b"not" => Group::Not,
            _ => return,
        };
        self.stack.push(Frame {
            kind,
            children: Vec::new(),
        });
    }

    /// The matching end tag: fold the group into its parent.
    pub(crate) fn close_group(&mut self, name: &[u8]) {
        if !matches!(name, b"and" | b"or" | b"not") || self.stack.len() < 2 {
            return;
        }
        let frame = self.stack.pop().expect("checked len");
        if let Some(node) = frame.fold() {
            self.push(node);
        }
    }

    /// A parsed `<player>` / `<target>` element. `None` — Java's null — is
    /// dropped here rather than stored, so an `<and>` never holds one; the
    /// only null that survives is a whole block's, which [`Self::finish`]
    /// turns into no block at all.
    pub(crate) fn push_leaf(&mut self, leaf: Option<Cond>) {
        if let Some(leaf) = leaf {
            self.push(leaf);
        }
    }

    fn push(&mut self, node: Cond) {
        if let Some(frame) = self.stack.last_mut() {
            frame.children.push(node);
        }
    }

    /// `</cond>` — the finished block, or `None` when nothing in it parsed
    /// (Java's `attachCondition(null)`, which `checkCondition` skips).
    pub(crate) fn finish(&mut self) -> Option<ItemCondition> {
        let root = self.stack.drain(..).next()?;
        // Anything still open is a malformed block; its frames go with the
        // drain above, so the next `<cond>` starts clean either way.
        self.stack.clear();
        Some(ItemCondition {
            node: root.fold()?,
            message: std::mem::take(&mut self.message),
        })
    }
}

impl Frame {
    fn fold(mut self) -> Option<Cond> {
        match self.kind {
            // `getFirstChild()`: one child, the rest dropped.
            Group::Root => self.children.drain(..).next(),
            Group::Not => self
                .children
                .drain(..)
                .next()
                .map(|c| Cond::Not(Box::new(c))),
            Group::And | Group::Or => {
                if self.children.is_empty() {
                    // Java logs `Empty <and> condition` and still returns the
                    // (empty) condition, which tests true for `and` and false
                    // for `or`. Keeping the empty node reproduces both.
                    return Some(match self.kind {
                        Group::Or => Cond::Or(Vec::new()),
                        _ => Cond::And(Vec::new()),
                    });
                }
                Some(match self.kind {
                    Group::Or => Cond::Or(self.children),
                    _ => Cond::And(self.children),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dist;

    fn attrs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn one_attribute_stays_bare_and_several_join_into_an_and() {
        assert_eq!(
            player_condition(&attrs(&[("sex", "1")])),
            Some(Cond::Sex(1)),
            "joinAnd leaves a single condition alone"
        );
        assert_eq!(
            player_condition(&attrs(&[("sex", "1"), ("level", "40")])),
            Some(Cond::And(vec![Cond::Sex(1), Cond::Level(40)])),
            "two attributes become ConditionLogicAnd, in document order"
        );
    }

    #[test]
    fn an_unknown_attribute_is_dropped_but_its_siblings_still_build() {
        // Java's switch has no default arm: `hairColour` matches nothing and
        // contributes nothing, while `races` on the same element still does.
        assert_eq!(
            player_condition(&attrs(&[("hairColour", "3"), ("races", "KAMAEL")])),
            Some(Cond::Race(vec![Race::Kamael]))
        );
        // …and an element with *nothing* recognised is Java's `null`, which
        // `checkCondition` skips rather than failing.
        assert_eq!(player_condition(&attrs(&[("hairColour", "3")])), None);
    }

    #[test]
    fn attribute_names_are_matched_case_insensitively() {
        // `DocumentBase` lowercases before the switch, which is the only
        // reason `SiegeZone` and `MinimumVitalityPoints` — spelled in mixed
        // case in every item that uses them — are recognised at all.
        assert_eq!(
            player_condition(&attrs(&[("SiegeZone", "126")])),
            Some(Cond::SiegeZone {
                value: 126,
                self_: true
            })
        );
        assert_eq!(
            player_condition(&attrs(&[("MinimumVitalityPoints", "35000")])),
            Some(Cond::MinimumVitalityPoints(35000))
        );
    }

    #[test]
    fn a_malformed_level_range_is_ignored_rather_than_halved() {
        // Java builds the condition only `if (range.length == 2)`.
        assert_eq!(player_condition(&attrs(&[("levelRange", "76")])), None);
        assert_eq!(
            player_condition(&attrs(&[("levelRange", "76;84")])),
            Some(Cond::LevelRange(76, 84))
        );
    }

    #[test]
    fn message_reads_msg_first_then_msg_id_with_add_name() {
        assert_eq!(
            message_from(Some("nope".into()), Some("113"), Some("1")),
            CondMessage::Text("nope".into()),
            "`msg` wins: Java's `else if`"
        );
        assert_eq!(
            message_from(None, Some("113"), Some("1")),
            CondMessage::Sm {
                id: 113,
                add_name: true
            }
        );
        assert_eq!(
            message_from(None, Some("113"), None),
            CondMessage::Sm {
                id: 113,
                add_name: false
            }
        );
        assert_eq!(message_from(None, None, Some("1")), CondMessage::Silent);
    }

    /// The whole `<cond>` surface of the shipped datapack, as one number per
    /// attribute. Every figure here was derived from the XML independently of
    /// the parser, so a silently-dropped arm shows up as a drop in one row
    /// rather than as nothing at all.
    #[test]
    fn the_dist_parses_to_the_blocks_the_datapack_declares() {
        let data = dist::items();
        let mut blocks = 0usize;
        let mut tally: std::collections::BTreeMap<&'static str, usize> = Default::default();
        for t in data.all() {
            for c in &t.pre_conditions {
                blocks += 1;
                for leaf in c.node.leaves() {
                    *tally.entry(leaf_name(leaf)).or_default() += 1;
                }
            }
        }
        assert_eq!(blocks, 2126, "<cond> blocks across stats/items");
        let expected: &[(&str, usize)] = &[
            ("races", 822),
            ("flyMounted", 450),
            ("categoryType", 261),
            ("level", 218),
            ("sex", 149),
            ("levelRange", 100),
            ("pledgeClass", 73),
            ("cloakStatus", 49),
            ("instanceId", 42),
            ("castle", 29),
            ("isHero", 29),
            ("SiegeZone", 22),
            ("class_id_restriction", 20),
            ("insideZoneId", 18),
            ("subclass", 17),
            ("pkCount", 16),
            ("clanHall", 8),
            ("chaotic", 6),
            ("MinimumVitalityPoints", 4),
            ("isOnSide", 4),
            ("isClanLeader", 2),
            ("vehicleMounted", 2),
            ("fort", 1),
            ("target levelRange", 1),
        ];
        for (name, count) in expected {
            assert_eq!(tally.get(name), Some(count), "{name} conditions");
        }
        assert_eq!(
            tally.len(),
            expected.len(),
            "no condition kind outside the expected set: {tally:?}"
        );
    }

    /// Three items read end to end, because the census above counts leaves
    /// without looking at how they nest or what they say.
    #[test]
    fn nesting_messages_and_the_one_target_condition_survive_the_parse() {
        let data = dist::items();
        // 9396 "Kamaelic Circlet" — a bare race gate with the "you do not meet
        // the required condition to equip that item" line and no `addName`.
        let kamael = data.get(9396).expect("item 9396");
        assert_eq!(kamael.pre_conditions.len(), 1);
        assert_eq!(
            kamael.pre_conditions[0].node,
            Cond::Race(vec![Race::Kamael])
        );
        assert_eq!(
            kamael.pre_conditions[0].message,
            CondMessage::Sm {
                id: 1518,
                add_name: false
            }
        );
        // 6902 "Pledge Shield" — an `<and>` of two `<player>` elements, which
        // is the shape a leaf-counting census cannot tell from two blocks.
        let shield = data.get(6902).expect("item 6902");
        assert_eq!(
            shield.pre_conditions.len(),
            1,
            "one block holding an <and>, not two blocks"
        );
        assert_eq!(
            shield.pre_conditions[0].node,
            Cond::And(vec![
                Cond::Race(vec![
                    Race::Human,
                    Race::Elf,
                    Race::DarkElf,
                    Race::Orc,
                    Race::Dwarf
                ]),
                Cond::HasClanHall(vec![-1]),
            ])
        );
        // 21708 is the only item with a `<target>` element on this dist.
        let target = data.get(21746).expect("item 21746");
        assert!(
            target
                .pre_conditions
                .iter()
                .any(|c| c.node.leaves().contains(&&Cond::TargetLevelRange(21, 85))),
            "the one <target levelRange> survives the parse"
        );
    }

    fn leaf_name(leaf: &Cond) -> &'static str {
        match leaf {
            Cond::Race(_) => "races",
            Cond::Level(_) => "level",
            Cond::LevelRange(..) => "levelRange",
            Cond::Chaotic(_) => "chaotic",
            Cond::IsHero(_) => "isHero",
            Cond::PkCount(_) => "pkCount",
            Cond::SiegeZone { .. } => "SiegeZone",
            Cond::IsClanLeader(_) => "isClanLeader",
            Cond::PledgeClass(_) => "pledgeClass",
            Cond::HasClanHall(_) => "clanHall",
            Cond::HasFort(_) => "fort",
            Cond::HasCastle(_) => "castle",
            Cond::Sex(_) => "sex",
            Cond::FlyMounted(_) => "flyMounted",
            Cond::VehicleMounted(_) => "vehicleMounted",
            Cond::ClassIdRestriction(_) => "class_id_restriction",
            Cond::Subclass(_) => "subclass",
            Cond::InstanceId(_) => "instanceId",
            Cond::CloakStatus(_) => "cloakStatus",
            Cond::InsideZoneId(_) => "insideZoneId",
            Cond::CategoryType(_) => "categoryType",
            Cond::IsOnSide(_) => "isOnSide",
            Cond::MinimumVitalityPoints(_) => "MinimumVitalityPoints",
            Cond::TargetLevelRange(..) => "target levelRange",
            Cond::And(_) | Cond::Or(_) | Cond::Not(_) => "logic",
        }
    }
}
