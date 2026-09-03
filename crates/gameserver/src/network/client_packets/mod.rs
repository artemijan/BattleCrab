//! Inbound (client → server) packets. Ported 1:1 from
//! `gameserver/network/clientpackets`. G1 covers only the transport handshake
//! packet `ProtocolVersion`; gameplay packets are parsed/dispatched on the game
//! thread from G2 on.
//!
//! # The base opcodes with no arm
//!
//! Java's `ClientPackets` wires 155 base opcodes. Seven of them have **no
//! behaviour to port** — each was checked on the Java side rather than inferred
//! from the absence of a handler here, and each is a deliberate non-port
//! (`SKIP(census)`), not a gap:
//!
//! - `MOVE_WITH_DELTA` (0x52) — the whole `runImpl` is `// TODO this`.
//! - `REQUEST_PLEDGE_EXTENDED_INFO` (0x66) — empty `runImpl`.
//! - `GAME_GUARD_REPLY` (0xCB) — validates a SHA and stores the result in
//!   `_isAuthedGG`, whose only reader `isAuthedGG()` is **called by nothing**.
//!   Unreachable as well as inert: `GameGuardQuery` is never sent, so the
//!   client is never asked.
//! - `REQUEST_REPLY_START_PLEDGE` (0x04), `REQUEST_REPLY_STOP_PLEDGE_WAR`
//!   (0x06), `REQUEST_REPLY_SURRENDER_PLEDGE_WAR` (0x08) — all three return
//!   immediately unless `getActiveRequester()` is set, and **nothing in the
//!   clan-war path ever sets it** (`onTransactionRequest` is called only by
//!   trade, duel, party-room, MPCC and friend invites). The war declarations
//!   act unilaterally, and both reachable routes to `ClanWarState::MUTUAL` —
//!   declaring back, and five kills — are ported in
//!   [`crate::game_loop::clans::wars`].
//! - `REQUEST_CHANGE_PET_NAME` (0x93) — guarded by `if (pet.getName() != null
//!   && !pet.getName().isEmpty())`, and neither `Pet` nor `Summon` overrides
//!   `Npc.getName()`, which returns `getTemplate().getName()`. All 873 pet
//!   templates on this dist have a name, so the guard is always true and every
//!   rename is refused with `YOU_CANNOT_SET_THE_NAME_OF_THE_PET`.

pub mod combat;
pub mod commerce;
pub mod ex_opcodes;
pub mod items;
pub mod movement;
pub mod opcodes;
pub mod party;
pub mod quests;
pub mod session;
pub mod shortcuts;
pub mod social;
