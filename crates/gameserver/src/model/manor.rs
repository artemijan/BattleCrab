//! Castle-manor runtime state — port of `CastleManorManager`'s per-castle
//! production/procure lists (`model/SeedProduction`, `model/CropProcure`). The
//! static seed catalogue lives in [`crate::data::manor_data`]; this is the
//! *live* state a castle owner sets up: how much of each seed the manor sells
//! (production) and how much of each crop it buys back (procure), for the
//! current and next manor period.
//!
//! Loaded at boot from `castle_manor_production` / `castle_manor_procure`
//! (`DbEvent::ManorLoaded`). The manor-period [`ManorMode`] (APPROVED /
//! MODIFIABLE / MAINTENANCE) is driven by the wall-clock scheduler in
//! [`crate::game_loop::manor`], which also runs the daily [`ManorState::roll_period`]
//! production rollover. The economic settlement that Java folds into the
//! rollover (crop payout to the clan warehouse, treasury refund/charge) is still
//! deferred — see the `TODO(manor)` in `advance_manor_mode`.

use std::collections::HashMap;

/// Java `model/SeedProduction` — one seed line the manor offers for sale.
#[derive(Debug, Clone)]
pub struct SeedProduction {
    /// Java `_seedId`.
    pub seed_id: i32,
    /// `_amount` — quantity left to sell (mutated as players buy seeds).
    pub amount: i64,
    pub price: i64,
    /// `_startAmount` — the quantity originally set up (the display's "total").
    pub start_amount: i64,
}

impl SeedProduction {
    /// Java `decreaseAmount`: subtract `value`, refusing (returning `false`)
    /// if it would go negative.
    pub fn decrease_amount(&mut self, value: i64) -> bool {
        if self.amount - value < 0 {
            return false;
        }
        self.amount -= value;
        true
    }
}

/// Java `model/CropProcure` (a `SeedProduction` plus a reward type) — one crop
/// line the manor buys back. The `id` here is the **crop** id.
#[derive(Debug, Clone)]
pub struct CropProcure {
    pub crop_id: i32,
    /// Quantity left to buy (mutated as players sell crops).
    pub amount: i64,
    pub price: i64,
    pub start_amount: i64,
    /// Java `_rewardType` (0/1/2) — which reward the crop is exchanged for.
    pub reward_type: i32,
}

impl CropProcure {
    pub fn decrease_amount(&mut self, value: i64) -> bool {
        if self.amount - value < 0 {
            return false;
        }
        self.amount -= value;
        true
    }
}

/// Java `enums/ManorMode` — the manor's period phase. Ordinals match Java (the
/// mode is driven by a wall-clock schedule at boot; the scheduler + period
/// rollover are not ported yet, so the mode stays at its default). `APPROVED`
/// is the Java field default (`_mode = ManorMode.APPROVED`): the settled state
/// where the manor sells/buys but the setup is locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ManorMode {
    Disabled,
    Modifiable,
    Maintenance,
    #[default]
    Approved,
}

/// Per-castle manor production/procure lists for the current and next period
/// (Java `CastleManorManager._production/_productionNext/_procure/_procureNext`,
/// keyed by castle id), plus the period [`ManorMode`].
#[derive(Debug, Default)]
pub struct ManorState {
    mode: ManorMode,
    production: HashMap<i32, Vec<SeedProduction>>,
    production_next: HashMap<i32, Vec<SeedProduction>>,
    procure: HashMap<i32, Vec<CropProcure>>,
    procure_next: HashMap<i32, Vec<CropProcure>>,
}

impl ManorState {
    /// Java `getManorMode` — the current period phase.
    pub fn mode(&self) -> ManorMode {
        self.mode
    }

    /// Set the period phase (Java's `scheduleModeChange` transitions; here
    /// driven by tests / a future scheduler).
    pub fn set_mode(&mut self, mode: ManorMode) {
        self.mode = mode;
    }

    /// Java `isManorApproved` — the settled phase (setup locked).
    pub fn is_manor_approved(&self) -> bool {
        self.mode == ManorMode::Approved
    }

    /// Java `isModifiablePeriod` — the owner may edit the next-period setup.
    pub fn is_modifiable_period(&self) -> bool {
        self.mode == ManorMode::Modifiable
    }

    /// Java `isUnderMaintenance` — the daily production/procure rollover window.
    pub fn is_under_maintenance(&self) -> bool {
        self.mode == ManorMode::Maintenance
    }

    /// Java `getSeedProduction(castleId, nextPeriod)`.
    pub fn seed_production(&self, castle_id: i32, next_period: bool) -> &[SeedProduction] {
        let map = if next_period {
            &self.production_next
        } else {
            &self.production
        };
        map.get(&castle_id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Java `getSeedProduct(castleId, seedId, nextPeriod)`.
    pub fn seed_product(
        &self,
        castle_id: i32,
        seed_id: i32,
        next_period: bool,
    ) -> Option<&SeedProduction> {
        self.seed_production(castle_id, next_period)
            .iter()
            .find(|s| s.seed_id == seed_id)
    }

    /// Java `getCropProcure(castleId, nextPeriod)`.
    pub fn crop_procure(&self, castle_id: i32, next_period: bool) -> &[CropProcure] {
        let map = if next_period {
            &self.procure_next
        } else {
            &self.procure
        };
        map.get(&castle_id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Java `getCropProcure(castleId, cropId, nextPeriod)`.
    pub fn crop_procure_for(
        &self,
        castle_id: i32,
        crop_id: i32,
        next_period: bool,
    ) -> Option<&CropProcure> {
        self.crop_procure(castle_id, next_period)
            .iter()
            .find(|c| c.crop_id == crop_id)
    }

    /// Install a castle's seed-production list for a period (boot load / setup).
    pub fn set_seed_production(
        &mut self,
        castle_id: i32,
        next_period: bool,
        list: Vec<SeedProduction>,
    ) {
        let map = if next_period {
            &mut self.production_next
        } else {
            &mut self.production
        };
        map.insert(castle_id, list);
    }

    /// Install a castle's crop-procure list for a period (boot load / setup).
    pub fn set_crop_procure(&mut self, castle_id: i32, next_period: bool, list: Vec<CropProcure>) {
        let map = if next_period {
            &mut self.procure_next
        } else {
            &mut self.procure
        };
        map.insert(castle_id, list);
    }

    /// Java `setNextSeedProduction(list, castleId)` — replace the castle's
    /// next-period seed setup (the owner's `RequestSetSeed`).
    pub fn set_next_seed_production(&mut self, castle_id: i32, list: Vec<SeedProduction>) {
        self.set_seed_production(castle_id, true, list);
    }

    /// Java `setNextCropProcure(list, castleId)` — replace the castle's
    /// next-period crop setup (the owner's `RequestSetCrop`).
    pub fn set_next_crop_procure(&mut self, castle_id: i32, list: Vec<CropProcure>) {
        self.set_crop_procure(castle_id, true, list);
    }

    /// Decrease a current-period seed's remaining amount (a player bought
    /// `value` of it). Returns `false` — leaving the amount unchanged — when the
    /// seed is absent or the buy would overdraw (Java `SeedProduction.decreaseAmount`).
    pub fn decrease_seed_amount(&mut self, castle_id: i32, seed_id: i32, value: i64) -> bool {
        self.production
            .get_mut(&castle_id)
            .and_then(|list| list.iter_mut().find(|s| s.seed_id == seed_id))
            .is_some_and(|sp| sp.decrease_amount(value))
    }

    /// Decrease a current-period crop's remaining amount (a player sold `value`
    /// of it). Same semantics as [`Self::decrease_seed_amount`].
    pub fn decrease_crop_amount(&mut self, castle_id: i32, crop_id: i32, value: i64) -> bool {
        self.procure
            .get_mut(&castle_id)
            .and_then(|list| list.iter_mut().find(|c| c.crop_id == crop_id))
            .is_some_and(|cp| cp.decrease_amount(value))
    }

    /// The daily production rollover (the data half of Java `changeMode`'s
    /// `APPROVED` case): the castle's next-period setup becomes current, and the
    /// next period is re-seeded from it with amounts reset to their start (a
    /// fresh full period). Java shares the `SeedProduction`/`CropProcure` objects
    /// between the two lists (a latent aliasing quirk); this port keeps them
    /// independent clones, which matches the intended "next starts fresh"
    /// semantics. The economic settlement (crop payout to the clan warehouse,
    /// treasury refund/charge, affordability gating) is **not** applied here —
    /// see the `TODO(manor)` at the caller.
    pub fn roll_period(&mut self, castle_id: i32) {
        let next_prod = self
            .production_next
            .get(&castle_id)
            .cloned()
            .unwrap_or_default();
        let next_proc = self
            .procure_next
            .get(&castle_id)
            .cloned()
            .unwrap_or_default();

        // Next → current (carrying whatever amounts the owner set up).
        self.production.insert(castle_id, next_prod.clone());
        self.procure.insert(castle_id, next_proc.clone());

        // Re-seed next period, amounts reset to their start (full).
        let fresh_prod = next_prod
            .into_iter()
            .map(|mut sp| {
                sp.amount = sp.start_amount;
                sp
            })
            .collect();
        let fresh_proc = next_proc
            .into_iter()
            .map(|mut cp| {
                cp.amount = cp.start_amount;
                cp
            })
            .collect();
        self.production_next.insert(castle_id, fresh_prod);
        self.procure_next.insert(castle_id, fresh_proc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_and_lookup_getters() {
        let mut m = ManorState::default();
        m.set_seed_production(
            1,
            false,
            vec![SeedProduction {
                seed_id: 5016,
                amount: 100,
                price: 3,
                start_amount: 100,
            }],
        );
        m.set_crop_procure(
            1,
            true,
            vec![CropProcure {
                crop_id: 5073,
                amount: 50,
                price: 9,
                start_amount: 50,
                reward_type: 1,
            }],
        );

        // Current-period seed production is set; next-period is empty.
        assert_eq!(m.seed_production(1, false).len(), 1);
        assert!(m.seed_production(1, true).is_empty());
        assert_eq!(m.seed_product(1, 5016, false).unwrap().amount, 100);
        assert!(m.seed_product(1, 9999, false).is_none());

        // Crop procure was set for the next period only.
        assert!(m.crop_procure(1, false).is_empty());
        assert_eq!(m.crop_procure_for(1, 5073, true).unwrap().reward_type, 1);

        // An unknown castle is empty, not a panic.
        assert!(m.seed_production(9, false).is_empty());
    }

    #[test]
    fn roll_period_promotes_next_and_resets() {
        let mut m = ManorState::default();
        // Next-period setup with a mid-period amount (300) below its start (500).
        m.set_next_seed_production(
            1,
            vec![SeedProduction {
                seed_id: 5016,
                amount: 300,
                price: 3,
                start_amount: 500,
            }],
        );
        assert!(
            m.seed_production(1, false).is_empty(),
            "current empty before roll"
        );

        m.roll_period(1);

        // Next → current, carrying its amount.
        let cur = m.seed_production(1, false);
        assert_eq!(cur.len(), 1);
        assert_eq!(cur[0].seed_id, 5016);
        assert_eq!(cur[0].amount, 300, "current carries the next-period amount");
        // Next is re-seeded with amounts reset to start (a fresh full period).
        let next = m.seed_production(1, true);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].amount, 500, "next resets to its start amount");
        // The two lists are independent (no Java-style aliasing).
        assert_ne!(cur[0].amount, next[0].amount);
    }

    #[test]
    fn decrease_amount_refuses_negative() {
        let mut s = SeedProduction {
            seed_id: 1,
            amount: 10,
            price: 1,
            start_amount: 10,
        };
        assert!(s.decrease_amount(4));
        assert_eq!(s.amount, 6);
        assert!(!s.decrease_amount(7), "would go negative → refused");
        assert_eq!(s.amount, 6, "amount unchanged on refusal");
    }
}
