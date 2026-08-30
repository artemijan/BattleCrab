//! Where a body is, what it can see and which volume it stands in — the
//! modules that all resolve against the world-region grid.
//!
//! Movement proper ([`position`], [`boats`]) and the hazards that come with it
//! ([`falling`], [`water`]); the zone membership a move revalidates
//! ([`zones`], [`effect_zones`]); who becomes visible to whom
//! ([`visibility`]); and the two ways a player leaves the shared map behind —
//! a private [`instances`] world and the free-look camera of [`observation`].

pub(crate) mod boats;
pub(crate) mod effect_zones;
pub(crate) mod falling;
pub(crate) mod instances;
pub(crate) mod observation;
pub(crate) mod position;
pub(in crate::game_loop) mod visibility;
pub(crate) mod water;
pub(crate) mod zones;
