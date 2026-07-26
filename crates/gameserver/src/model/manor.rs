//! Castle-manor runtime state — port of `CastleManorManager`'s per-castle
//! production/procure lists (`model/SeedProduction`, `model/CropProcure`). The
//! static seed catalogue lives in [`crate::data::manor_data`]; this is the
//! *live* state a castle owner sets up: how much of each seed the manor sells
//! (production) and how much of each crop it buys back (procure), for the
//! current and next manor period.
//!
//! Loaded at boot from `castle_manor_production` / `castle_manor_procure`
//! (`DbEvent::ManorLoaded`). The manor-period **mode** (APPROVED / MODIFIABLE /
//! MAINTENANCE, driven by a wall-clock schedule) and the period rollover are
//! not modelled yet — TODO(manor): they gate the owner *setup* views (requests
//! 7/8) and the daily production reset, a later slice.

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

/// Per-castle manor production/procure lists for the current and next period
/// (Java `CastleManorManager._production/_productionNext/_procure/_procureNext`,
/// keyed by castle id).
#[derive(Debug, Default)]
pub struct ManorState {
    production: HashMap<i32, Vec<SeedProduction>>,
    production_next: HashMap<i32, Vec<SeedProduction>>,
    procure: HashMap<i32, Vec<CropProcure>>,
    procure_next: HashMap<i32, Vec<CropProcure>>,
}

impl ManorState {
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
