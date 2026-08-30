//! The autopilot family — the client-driven loops that keep playing for a
//! player who is not touching the keyboard: `.play` combat automation, the
//! `.playskills`/`.playitems` auto-use panel and the auto-potion watchdog.
//!
//! Java splits these across `taskmanager/AutoPlayTaskManager`,
//! `AutoUseTaskManager` and `AutoPotionTaskManager`; they share the settings
//! components on the player and the same "act once per N ticks" shape.

pub(crate) mod play;
pub(crate) mod potions;
pub(crate) mod use_items;
