//! Grand Olympiad (G25) — the state model and noble registry.
//! Java `model/olympiad/{Olympiad, OlympiadManager}`.
//!
//! Slices 1–2 cover the noble records, the registration queues and the Grand
//! Olympiad Manager NPC dialog (`scripts/oly_manager.rs`). The daily
//! competition-window / weekly / monthly period scheduling, DB persistence,
//! match execution and hero calculation are later slices (see
//! `docs/PLAN_G25_OLYMPIAD.md`).

use std::collections::{HashMap, HashSet};

/// Which match queue a noble registers for (Java `CompetitionType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompetitionType {
    /// A 1v1 against the same class group (`_classBasedRegisters`).
    Classed,
    /// A 1v1 against anyone, class-irrelevant (`_nonClassBasedRegisters`).
    NonClassed,
}

/// A running 1v1 match (Java an `OlympiadGameNormal` attached to a stadium).
#[derive(Debug, Clone)]
pub struct OlympiadMatch {
    /// The stadium index the match runs in (Java the `OlympiadGameTask` slot).
    pub arena: usize,
    pub player_a: i32,
    pub player_b: i32,
    /// Game tick at which an undecided battle is called a draw (Java
    /// `ALT_OLY_BATTLE`).
    pub deadline_tick: u64,
    /// Where each fighter was before the match, to port them back afterward.
    pub return_a: (i32, i32, i32),
    pub return_b: (i32, i32, i32),
}

/// A noble's persistent Olympiad record (Java `Olympiad.NOBLES` `StatSet`).
#[derive(Debug, Clone)]
pub struct NobleStats {
    pub class_id: i32,
    pub name: String,
    pub points: i32,
    pub comp_done: i32,
    pub comp_won: i32,
    pub comp_lost: i32,
    pub comp_drawn: i32,
    pub comp_done_week: i32,
}

impl NobleStats {
    /// A noble's very first record: the starting points, no matches yet
    /// (Java `registerNoble`'s fresh `StatSet`).
    pub fn fresh(class_id: i32, name: String) -> Self {
        Self {
            class_id,
            name,
            points: DEFAULT_POINTS,
            comp_done: 0,
            comp_won: 0,
            comp_lost: 0,
            comp_drawn: 0,
            comp_done_week: 0,
        }
    }
}

// Tunables — dist `config/Olympiad.ini` (authoritative). Hard-coded here in
// slice 1; live-config plumbing is a later concern.
/// `AltOlyStartPoints` — a noble's starting Olympiad points.
pub const DEFAULT_POINTS: i32 = 10;
/// `AltOlyMaxWeeklyMatches` — matches a noble may enter per week.
pub const MAX_WEEKLY_MATCHES: i32 = 30;
/// `AltOlyClassedParticipants` — cap per class-based queue.
pub const CLASSED_PARTICIPANTS: usize = 20;
/// `AltOlyNonClassedParticipants` — cap on the non-class queue.
pub const NONCLASSED_PARTICIPANTS: usize = 20;
/// Registration closes this long before the competition window ends (Java
/// `getMillisToCompEnd() < 1_200_000`, i.e. 20 minutes).
pub const REG_CLOSE_BEFORE_END_MS: u64 = 1_200_000;

/// The Grand Olympiad Manager NPC (`ai/others/OlyManager` `MANAGER`).
pub const OLYMPIAD_MANAGER_NPC: i32 = 31688;

/// The Olympiad's live state (Java `Olympiad` singleton fields + the
/// `OlympiadManager` registration queues), held on `World`.
#[derive(Debug, Default)]
pub struct OlympiadState {
    /// 0 = competition period (matches run); 1 = validation period (heroes
    /// crowned, no registration). Java `Olympiad._period`.
    pub period: i32,
    /// The monthly cycle number (Java `_currentCycle`).
    pub current_cycle: i32,
    /// Whether the daily competition window is open (Java `_inCompPeriod`).
    pub in_comp_period: bool,
    /// The game tick at which the current competition window closes; the
    /// "registration closed" gate compares against it.
    pub comp_end_tick: u64,
    /// Persisted wall-clock milestones (Java `olympiad_data`): the current
    /// month's end, the validation-period end, and the next weekly refresh.
    /// Loaded/saved as-is; the window scheduler (a later slice) consumes them.
    pub olympiad_end: i64,
    pub validation_end: i64,
    pub next_weekly_change: i64,
    /// Every noble who has ever registered, by character object id.
    pub nobles: HashMap<i32, NobleStats>,
    /// Object ids waiting in the class-irrelevant queue.
    pub non_class_registers: HashSet<i32>,
    /// Object ids waiting in each class-group queue, keyed by class group.
    pub class_registers: HashMap<i32, HashSet<i32>>,
    /// The matches currently running, one per busy stadium (Java the
    /// `OlympiadGameManager._tasks` with an attached game).
    pub matches: Vec<OlympiadMatch>,
    /// Everyone currently in a running match (Java `isInCompetition`): they
    /// can't register or unregister while fighting.
    pub in_competition: HashSet<i32>,
    /// The heroes crowned for the current cycle — `(char_id, hero_class_id)`
    /// (Java the `heroes` table). Recomputed at each olympiad end.
    pub heroes: Vec<(i32, i32)>,
}

impl OlympiadState {
    /// The class-group key a player competes within (Java
    /// `OlympiadManager.getClassGroup`). The later-chronicle `SIXTH_*` /
    /// `ERTHEIA_*` category branches are unreachable in Interlude, so this is
    /// always the base class id.
    pub fn class_group(base_class_id: i32) -> i32 {
        base_class_id
    }

    /// A noble's points, creating the record with the starting points if this
    /// is the first time it is asked for (Java `getNoblePoints`).
    pub fn noble_points_or_create(
        &mut self,
        object_id: i32,
        base_class_id: i32,
        name: &str,
    ) -> i32 {
        self.nobles
            .entry(object_id)
            .or_insert_with(|| NobleStats::fresh(base_class_id, name.to_string()))
            .points
    }

    /// Java `OlympiadManager.getCountOpponents` — the non-class registrants plus
    /// the number of populated class groups (Java sums a Set size and a Map size;
    /// this build only uses the non-class list, so it is that count).
    pub fn count_opponents(&self) -> usize {
        self.non_class_registers.len() + self.class_registers.len()
    }

    /// Matches a noble may still enter this week (Java
    /// `getRemainingWeeklyMatches`). An unknown noble has the full allowance.
    pub fn remaining_weekly_matches(&self, object_id: i32) -> i32 {
        let done = self.nobles.get(&object_id).map_or(0, |n| n.comp_done_week);
        (MAX_WEEKLY_MATCHES - done).max(0)
    }

    /// Whether `object_id` is currently fighting a match (Java
    /// `OlympiadManager.isInCompetition`).
    pub fn is_in_competition(&self, object_id: i32) -> bool {
        self.in_competition.contains(&object_id)
    }

    /// Whether `object_id` sits in either registration queue (Java
    /// `OlympiadManager.isRegistered`).
    pub fn is_registered(&self, object_id: i32) -> bool {
        self.non_class_registers.contains(&object_id)
            || self
                .class_registers
                .values()
                .any(|set| set.contains(&object_id))
    }

    /// Drop `object_id` from whichever queue holds it; returns the queue it was
    /// removed from, or `None` if it was not registered.
    pub fn remove_registration(&mut self, object_id: i32) -> Option<CompetitionType> {
        if self.non_class_registers.remove(&object_id) {
            return Some(CompetitionType::NonClassed);
        }
        for set in self.class_registers.values_mut() {
            if set.remove(&object_id) {
                return Some(CompetitionType::Classed);
            }
        }
        None
    }
}
