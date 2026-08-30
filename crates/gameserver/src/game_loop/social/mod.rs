//! Talking to other players and choosing who may talk back — the chat
//! channels and their `.` voiced commands ([`chat`], which owns the per-player
//! block list) and the friend roster ([`friends`]).

pub(crate) mod chat;
pub(in crate::game_loop) mod friends;
