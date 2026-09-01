//! The character behind the avatar — the state a player accumulates on their
//! own account rather than in the world around them: subclasses, henna dyes,
//! vitality, recommendations, PC-café points and the sitting pose.
//!
//! Combat numbers derived from any of this live in
//! [`crate::game_loop::stats`]; what lives here is the state itself and the
//! packets that let a player change it.

pub(in crate::game_loop) mod henna;
pub mod inventory;
pub(crate) mod pc_cafe;
pub(crate) mod player_info;
pub(in crate::game_loop) mod reco;
pub(in crate::game_loop) mod sit_stand;
pub(crate) mod subclass;
pub(in crate::game_loop) mod vitality;
