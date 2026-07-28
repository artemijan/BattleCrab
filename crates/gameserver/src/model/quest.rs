//! Per-player quest progress — port of `model/quest/QuestState.java` +
//! `State.java`, the data half only. The engine that mutates it (talk/event/
//! kill dispatch, packet sends, persistence commands) lives in
//! `game_loop/quests.rs`; this module is pure state + the cond bookkeeping
//! math, unit-testable without a `World`.
//!
//! DB shape (Java-compatible, same rows either server can read):
//! `character_quests(charId, name, var, value)` — one row per variable, the
//! state stored under the `"<state>"` var as `Start`/`Started`/`Completed`.

use std::collections::HashMap;

/// `State.CREATED/STARTED/COMPLETED` + the DB string names
/// (`State.getStateName`/`getStateId` — note CREATED persists as `"Start"`,
/// not `"Created"`).
pub mod state {
    pub const CREATED: u8 = 0;
    pub const STARTED: u8 = 1;
    pub const COMPLETED: u8 = 2;

    pub fn name(state: u8) -> &'static str {
        match state {
            STARTED => "Started",
            COMPLETED => "Completed",
            _ => "Start",
        }
    }

    pub fn from_name(name: &str) -> u8 {
        match name {
            "Started" => STARTED,
            "Completed" => COMPLETED,
            _ => CREATED,
        }
    }
}

/// The reserved variable names (`QuestState.COND_VAR` etc.).
pub const STATE_VAR: &str = "<state>";
pub const COND_VAR: &str = "cond";
/// The skipped-step bitset var maintained by [`updated_cond_flags`].
pub const FLAGS_VAR: &str = "__compltdStateFlags";
/// `QuestState.MEMO_VAR` — the Mobius scripts' step variable (Q255 Tutorial).
pub const MEMO_VAR: &str = "memoState";

/// One quest's progress on one player (`QuestState`): the state byte plus
/// the string-keyed variable map (`cond`, `memoState`, script-specific keys).
#[derive(Debug, Clone, Default)]
pub struct QuestState {
    pub state: u8,
    pub vars: HashMap<String, String>,
}

impl QuestState {
    pub fn is_created(&self) -> bool {
        self.state == state::CREATED
    }

    pub fn is_started(&self) -> bool {
        self.state == state::STARTED
    }

    pub fn is_completed(&self) -> bool {
        self.state == state::COMPLETED
    }

    /// `QuestState.getInt`: missing/empty/non-numeric parse as 0.
    pub fn get_int(&self, var: &str) -> i32 {
        self.vars.get(var).and_then(|v| v.parse().ok()).unwrap_or(0)
    }

    /// `QuestState.getCond`: the current step, 0 unless STARTED.
    pub fn cond(&self) -> i32 {
        if self.is_started() {
            self.get_int(COND_VAR)
        } else {
            0
        }
    }

    /// `QuestState.isCond`.
    pub fn is_cond(&self, condition: i32) -> bool {
        self.cond() == condition
    }

    /// `QuestState.getCondBitSet` — the value `QuestList` sends per active
    /// quest. Normally the plain cond int; the bit-31 branch unpacks legacy
    /// rows where the cond column itself holds a skipped-step flag word
    /// (pre-Mobius servers stored it there) down to the highest step.
    pub fn cond_bit_set(&self) -> i32 {
        if !self.is_started() {
            return 0;
        }
        let mut val = self.get_int(COND_VAR);
        if (val as u32 & 0x8000_0000) != 0 {
            val &= 0x7fff_ffff;
            for i in 1..32 {
                val >>= 1;
                if val == 0 {
                    val = i;
                    break;
                }
            }
        }
        val
    }
}

/// The `__compltdStateFlags` maintenance from the private
/// `QuestState.setCond(cond, old)` as a pure function over the stored var:
/// `stored` is the current value (`None` = var absent), the result is what
/// the var must become (`None` = delete it). Java interleaves unset/set
/// calls; the net effect is a single transition, reproduced branch-for-
/// branch below. Shifts use `wrapping_shl` to match Java's mod-32 `<<`.
pub fn updated_cond_flags(cond: i32, old: i32, stored: Option<i32>) -> Option<i32> {
    if cond == old {
        return stored;
    }
    // cond 0-2 never need flags (step 1 can't be skipped) and >31 can't be
    // represented: Java unsets the var up front and works from flags = 0.
    let out_of_range = !(3..=31).contains(&cond);
    let mut flags = if out_of_range { 0 } else { stored.unwrap_or(0) };

    if flags == 0 {
        // Case 1: nothing skipped so far. A jump past old+1 skips for the
        // first time: bit 31 marks "skips exist", bits 0..old mark the
        // contiguously completed steps, bit cond-1 the current one.
        if cond > old + 1 {
            flags = 0x8000_0001u32 as i32
                | 1i32.wrapping_shl(old as u32).wrapping_sub(1)
                | 1i32.wrapping_shl((cond - 1) as u32);
            Some(flags)
        } else if out_of_range {
            None
        } else {
            stored
        }
    } else if cond < old {
        // Case 2, pushed back to an earlier step: clear everything ahead;
        // if that leaves a contiguous prefix there are no skips any more.
        flags &= 1i32.wrapping_shl(cond as u32).wrapping_sub(1);
        if flags == 1i32.wrapping_shl(cond as u32).wrapping_sub(1) {
            None
        } else {
            Some(flags | 0x8000_0001u32 as i32)
        }
    } else {
        // Case 3, forward with existing skips: just mark this step.
        Some(flags | 1i32.wrapping_shl((cond - 1) as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started_with_cond(cond: &str) -> QuestState {
        let mut qs = QuestState {
            state: state::STARTED,
            ..Default::default()
        };
        qs.vars.insert(COND_VAR.into(), cond.into());
        qs
    }

    #[test]
    fn state_names_round_trip_with_javas_start_quirk() {
        assert_eq!(state::name(state::CREATED), "Start");
        assert_eq!(state::name(state::STARTED), "Started");
        assert_eq!(state::name(state::COMPLETED), "Completed");
        for s in [state::CREATED, state::STARTED, state::COMPLETED] {
            assert_eq!(state::from_name(state::name(s)), s);
        }
        assert_eq!(state::from_name("garbage"), state::CREATED);
    }

    #[test]
    fn cond_reads_zero_unless_started() {
        let mut qs = started_with_cond("2");
        assert_eq!(qs.cond(), 2);
        assert!(qs.is_cond(2));
        qs.state = state::COMPLETED;
        assert_eq!(qs.cond(), 0);
        qs.state = state::CREATED;
        assert_eq!(qs.cond(), 0);
    }

    #[test]
    fn cond_bit_set_passes_plain_conds_and_unpacks_legacy_flag_words() {
        assert_eq!(started_with_cond("3").cond_bit_set(), 3);
        // Legacy row: flags 0x80000007 = steps 1-3 done, no skips beyond →
        // highest step 3 (traced from the Java loop).
        assert_eq!(
            started_with_cond(&(0x8000_0007u32 as i32).to_string()).cond_bit_set(),
            3
        );
        // 0x80000001 | 1<<4 → steps {1, 5}: loop exits at i = 5.
        let w = 0x8000_0001u32 as i32 | 1 << 4;
        assert_eq!(started_with_cond(&w.to_string()).cond_bit_set(), 5);
    }

    #[test]
    fn cond_flags_stay_absent_for_sequential_progress() {
        // The normal 1 → 2 → 3 march never materializes the var.
        assert_eq!(updated_cond_flags(1, 0, None), None);
        assert_eq!(updated_cond_flags(2, 1, None), None);
        assert_eq!(updated_cond_flags(3, 2, None), None);
    }

    #[test]
    fn cond_flags_record_a_skip_then_fill_and_clear() {
        // 2 → 5 skips 3 and 4: bit 31 + bits {0,1} (done) + bit 4 (current).
        let skipped = updated_cond_flags(5, 2, None);
        assert_eq!(skipped, Some(0x8000_0001u32 as i32 | 0b10 | 1 << 4));
        // Moving forward keeps marking steps.
        let fwd = updated_cond_flags(6, 5, skipped);
        assert_eq!(fwd, Some(skipped.unwrap() | 1 << 5));
        // Pushing back to 2: conds below 3 never carry flags (step 1 can't
        // be skipped) → var deleted via the out-of-range unset.
        assert_eq!(updated_cond_flags(2, 6, fwd), None);
    }

    #[test]
    fn cond_flags_pushback_keeps_remaining_skips() {
        // 1 → 6 skips 2-5 (bits {0, 5} + bit 31); back to 4 masks to bits
        // 0..4, leaving only bit 0 — not the full prefix, so steps 2-3 are
        // still recorded as skipped.
        let skipped = updated_cond_flags(6, 1, None).unwrap();
        let back = updated_cond_flags(4, 6, Some(skipped));
        assert_eq!(back, Some((skipped & 0b1111) | 0x8000_0001u32 as i32));
    }

    #[test]
    fn cond_flags_out_of_range_conds_drop_the_var() {
        // cond 2 after a skip-word existed (e.g. abort/restart paths).
        assert_eq!(updated_cond_flags(2, 5, Some(0x8000_0011u32 as i32)), None);
        assert_eq!(
            updated_cond_flags(32, 31, Some(0x8000_0011u32 as i32)),
            None
        );
    }
}
