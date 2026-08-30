//! The inbound-request surface: everything that turns a decoded client packet
//! into a call on a gameplay module.
//!
//! [`crate::game_loop::net`] owns the socket and the session below this;
//! `client` is the layer above it — opcode dispatch, the HTML `bypass` router,
//! the `/command` bar, the `RequestActionUse` family, the lobby stretch from
//! AuthLogin to EnterWorld, the shortcut/macro and UI-settings panels, and the
//! flood protector that rate-limits the lot.

pub(in crate::game_loop) mod actions;
pub(in crate::game_loop) mod bypass;
pub(in crate::game_loop) mod dispatch;
pub(crate) mod flood;
pub(in crate::game_loop) mod lobby;
pub(in crate::game_loop) mod settings;
pub(in crate::game_loop) mod shortcuts;
pub(in crate::game_loop) mod user_commands;
