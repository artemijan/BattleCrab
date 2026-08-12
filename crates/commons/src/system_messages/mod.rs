//! System messages: the ids the server sends and the values it sends with them.
//!
//! A system message on the wire is an id plus a list of typed parameters. The
//! client owns everything else — the wording, the colour, the font — looked up
//! in its own `SystemMsg*.dat`. That is why nothing here sends text: changing
//! a string in [`generated::ALL`] changes nothing until the client table is
//! rebuilt from it with `l2r-tools client-dat sync-messages`.
//!
//! [`generated`] is produced by `l2r-tools gen-messages` (`tools::msg_gen`)
//! from the Java reference plus the client table. Messages that take
//! parameters are types whose constructor takes exactly the right number of
//! arguments; the rest are constants.
//!
//! ```ignore
//! use commons::system_messages::generated::*;
//!
//! send(C1_HAS_INFLICTED_S3_DAMAGE_ON_C2::new(attacker, victim, damage));
//! send(YOU_CANNOT_MOVE_WHILE_CASTING);
//! ```
//!
//! # What is and is not checked
//!
//! The *number* of parameters is enforced, because it is derivable from the
//! message name. Their *types* are not: `$s1` renders as whatever type byte it
//! is sent with, so the same slot is an int in one call and an item in the
//! next. `S` positions therefore take a [`SmValue`], and only `C` positions —
//! which really are always names — are narrowed to [`Subject`].

pub mod generated;

/// One parameter of a system message, as the packet carries it.
///
/// The discriminants are the client's `TYPE_*` values and are part of the wire
/// format, not an internal choice.
#[derive(Clone, Debug, PartialEq)]
pub enum SmParam {
    /// `TYPE_TEXT` (0)
    Text(String),
    /// `TYPE_INT_NUMBER` (1)
    Int(i32),
    /// `TYPE_NPC_NAME` (2) — the writer adds the 1000000 the client expects.
    NpcName(i32),
    /// `TYPE_ITEM_NAME` (3)
    ItemName(i32),
    /// `TYPE_SKILL_NAME` (4)
    SkillName { id: i32, level: i32 },
    /// `TYPE_CASTLE_NAME` (5)
    CastleName(i32),
    /// `TYPE_LONG_NUMBER` (6)
    Long(i64),
    /// `TYPE_ZONE_NAME` (7) — the *client* resolves the region from the
    /// coordinates; there is no server-side lookup.
    ZoneName { x: i32, y: i32, z: i32 },
    /// `TYPE_PLAYER_NAME` (12)
    PlayerName(String),
    /// `TYPE_SYSTEM_STRING` (13) — an id into `SysString*.dat`.
    SysString(i32),
    /// `TYPE_POPUP_ID` (16) — the floating damage number over a head.
    Popup {
        target: i32,
        attacker: i32,
        damage: i32,
    },
}

/// A `$c<n>` slot: something with a name.
///
/// A bare name goes as [`SmParam::Text`], not [`SmParam::PlayerName`]. That
/// looks wrong and is not: Java fills `$c1` with `addString(target.getName())`
/// wherever the subject is whatever happened to be targeted — see
/// `Formulas.calcMagicSuccess` for `C1_HAS_RESISTED_YOUR_S2`, the retail twin
/// of our resist line. `PlayerName` is a different parameter type the client
/// treats specially, and sending it where a plain string is expected stops the
/// message rendering.
#[derive(Clone, Debug)]
pub enum Subject {
    /// A name already resolved to text — what `$c1` normally carries.
    Name(String),
    /// `TYPE_PLAYER_NAME`, for the messages that genuinely want it.
    Player(String),
    /// `TYPE_NPC_NAME`, resolved by the client from a template id.
    Npc(i32),
}

impl Subject {
    pub fn into_param(self) -> SmParam {
        match self {
            Subject::Name(name) => SmParam::Text(name),
            Subject::Player(name) => SmParam::PlayerName(name),
            Subject::Npc(id) => SmParam::NpcName(id),
        }
    }
}

impl From<&str> for Subject {
    fn from(name: &str) -> Self {
        Subject::Name(name.to_string())
    }
}

impl From<String> for Subject {
    fn from(name: String) -> Self {
        Subject::Name(name)
    }
}

/// A `$s<n>` slot. The client renders whatever it is sent, so this is a choice
/// the caller makes rather than something the message dictates.
#[derive(Clone, Debug)]
pub enum SmValue {
    Text(String),
    Int(i32),
    Long(i64),
    Item(i32),
    Skill { id: i32, level: i32 },
    Castle(i32),
    SysString(i32),
    Zone { x: i32, y: i32, z: i32 },
}

impl SmValue {
    pub fn into_param(self) -> SmParam {
        match self {
            SmValue::Text(s) => SmParam::Text(s),
            SmValue::Int(v) => SmParam::Int(v),
            SmValue::Long(v) => SmParam::Long(v),
            SmValue::Item(id) => SmParam::ItemName(id),
            SmValue::Skill { id, level } => SmParam::SkillName { id, level },
            SmValue::Castle(id) => SmParam::CastleName(id),
            SmValue::SysString(id) => SmParam::SysString(id),
            SmValue::Zone { x, y, z } => SmParam::ZoneName { x, y, z },
        }
    }
}

impl From<&str> for SmValue {
    fn from(s: &str) -> Self {
        SmValue::Text(s.to_string())
    }
}

impl From<String> for SmValue {
    fn from(s: String) -> Self {
        SmValue::Text(s)
    }
}

impl From<i32> for SmValue {
    fn from(v: i32) -> Self {
        SmValue::Int(v)
    }
}

impl From<i64> for SmValue {
    fn from(v: i64) -> Self {
        SmValue::Long(v)
    }
}

/// A message ready to send: an id and its parameters.
#[derive(Clone, Debug)]
pub struct SystemMessage {
    pub id: i32,
    pub params: Vec<SmParam>,
}

impl SystemMessage {
    pub fn new(id: i32, params: Vec<SmParam>) -> Self {
        SystemMessage { id, params }
    }
}

/// A message that takes no parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemMessageId(pub i32);

impl SystemMessageId {
    pub const fn new(id: i32) -> Self {
        SystemMessageId(id)
    }
}

impl From<SystemMessageId> for SystemMessage {
    fn from(id: SystemMessageId) -> Self {
        SystemMessage::new(id.0, Vec::new())
    }
}

/// A row of the client table, for rebuilding `SystemMsg*.dat`.
pub struct MessageInfo {
    pub id: i32,
    pub name: &'static str,
    pub text: &'static str,
    /// `RRGGBBAA`.
    ///
    /// The dat itself stores the channels **B, G, R, A** — a colour written
    /// `FF0000..` there draws blue, not red. Everything in Rust is RGBA and
    /// the swap happens only where the file is read or written, so a colour
    /// here reads the way it looks.
    pub color: &'static str,
    /// How many parameters the name implies.
    pub params: usize,
    /// Added by this server rather than shipped by the client.
    pub custom: bool,
    /// `group` column, when this message dictates one.
    ///
    /// Together with [`Self::msg_type`] this picks how the client draws the
    /// line. A `[none]`/`0` row renders in the default grey whatever the
    /// colour column says, which is why a custom message needs to name the
    /// class it belongs to rather than take the file's modal value.
    pub group: Option<i32>,
    /// `type` column (`[damage]`, `[server]`, `[none]`, …), when dictated.
    pub msg_type: Option<&'static str>,
}

/// The `C<n>`/`S<n>` tokens in a message name, in the order they appear.
///
/// The name is the only description a message has of its own parameters, and
/// Java's `parseMessageParameters` reads it exactly this way. Yields the kind
/// byte (`b'C'` or `b'S'`) and the 1-based position it labels; a name mentions
/// its tokens in sentence order, so the positions do not arrive sorted.
pub fn param_tokens(name: &str) -> impl Iterator<Item = (u8, usize)> {
    let bytes = name.as_bytes();
    (0..bytes.len().saturating_sub(1)).filter_map(move |i| {
        let (kind, digit) = (bytes[i], bytes[i + 1]);
        ((kind == b'C' || kind == b'S') && digit.is_ascii_digit())
            .then(|| (kind, (digit - b'0') as usize))
    })
}

/// How many parameters a name implies — its highest `C<n>`/`S<n>`.
pub fn arity(name: &str) -> usize {
    param_tokens(name).map(|(_, pos)| pos).max().unwrap_or(0)
}

/// Look a message up by id — for tooling and diagnostics, not the send path.
pub fn by_id(id: i32) -> Option<&'static MessageInfo> {
    generated::ALL.iter().find(|m| m.id == id)
}

/// Look a message up by its generated name.
pub fn by_name(name: &str) -> Option<&'static MessageInfo> {
    generated::ALL.iter().find(|m| m.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_populated_and_ids_are_unique() {
        assert!(generated::ALL.len() > 5000, "{}", generated::ALL.len());
        let mut ids: Vec<i32> = generated::ALL.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate message ids in the table");
    }

    #[test]
    fn names_are_unique_too() {
        let mut names: Vec<&str> = generated::ALL.iter().map(|m| m.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate message names");
    }

    /// The count in the table must match what the name implies, since that is
    /// what the generated constructors enforce.
    #[test]
    fn declared_param_counts_match_the_names() {
        for m in generated::ALL {
            assert_eq!(m.params, arity(m.name), "{}", m.name);
        }
    }

    /// The bug this guards: `$c1` sent as `PlayerName` rather than `Text`
    /// stopped the line rendering, which looked like a colour problem.
    #[test]
    fn a_bare_name_goes_as_text_the_way_java_sends_it() {
        assert_eq!(
            Subject::from("Test Gremlin").into_param(),
            SmParam::Text("Test Gremlin".to_string())
        );
        // The specific types stay available for messages that want them.
        assert_eq!(
            Subject::Player("Bob".into()).into_param(),
            SmParam::PlayerName("Bob".to_string())
        );
        assert_eq!(Subject::Npc(20001).into_param(), SmParam::NpcName(20001));
    }

    /// The half-damage magic-resist line, sent target-first exactly as
    /// `Formulas.calcMagicDam` does. Both slots are plain names.
    #[test]
    fn the_damage_decreased_message_takes_target_then_attacker() {
        let sm = generated::DAMAGE_IS_DECREASED_BECAUSE_C1_RESISTED_C2_S_MAGIC::new(
            "Target", "Attacker",
        );
        assert_eq!(sm.id, 2280);
        assert_eq!(
            sm.params,
            vec![
                SmParam::Text("Target".to_string()),
                SmParam::Text("Attacker".to_string()),
            ]
        );
    }

    #[test]
    fn a_zero_param_constant_becomes_a_sendable_message() {
        let sm: SystemMessage = generated::YOU_CANNOT_MOVE_WHILE_CASTING.into();
        assert!(sm.params.is_empty());
        assert_eq!(sm.id, generated::YOU_CANNOT_MOVE_WHILE_CASTING.0);
    }

    /// The custom debuff-feedback messages must survive regeneration.
    /// A literal percent is `%%`: a single `%` starts a format escape and the
    /// client mis-renders the line, which is what kept the debuff feedback
    /// looking wrong. 26 of the 27 retail messages that use one double it —
    /// the odd one out (`...to 100%. ...$s1%%...`) is inconsistent with
    /// itself, so this holds only the messages we author to the rule.
    #[test]
    fn a_literal_percent_is_doubled_in_messages_we_author() {
        for m in generated::ALL.iter().filter(|m| m.custom) {
            let bytes = m.text.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'%' {
                    assert_eq!(
                        bytes.get(i + 1),
                        Some(&b'%'),
                        "{}: lone `%` in {:?}",
                        m.name,
                        m.text
                    );
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }

    #[test]
    fn the_custom_messages_are_present_and_flagged() {
        for name in [
            "C1_HAS_RESISTED_S2_CHANCE_WAS_S3",
            "S1_LANDED_ON_C2_CHANCE_WAS_S3",
        ] {
            let info = by_name(name).unwrap_or_else(|| panic!("{name} missing"));
            assert!(info.custom, "{name} should be marked custom");
            // Above every retail id, but only just: the client does not seem
            // to accept one far beyond the highest it loaded.
            let highest_retail = generated::ALL
                .iter()
                .filter(|m| !m.custom)
                .map(|m| m.id)
                .max()
                .unwrap();
            assert!(
                info.id > highest_retail && info.id <= highest_retail + 10,
                "{name} id {} should continue the retail sequence after {highest_retail}",
                info.id
            );
            assert_eq!(info.params, 3);
        }
    }
}
