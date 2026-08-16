//! The dist catalogues, loaded once per test binary.
//!
//! `load_from` walks `dist/game/data` and parses the XML from scratch every
//! call — 627 ms for `SkillData`, ~170 ms each for `ItemData` and `NpcData`,
//! 1.15 s for a whole `GameData` — and several hundred call sites across these
//! tests were each paying that again. Cloning an already-parsed catalogue is
//! 16–35× cheaper (4.6 ms for `ItemData`, 37 ms for `SkillData`), so a test
//! that needs its own mutable copy (`insert_for_test`, or handing it to
//! `world.data`) takes a clone, and one that only reads borrows the `'static`
//! directly.
//!
//! `LazyLock` parses on first use, so a filtered `cargo test` run that touches
//! none of these still pays nothing. It shares nothing between tests, though —
//! nextest gives each test its own process — which is why the first load in
//! each process goes through [`snapshot`](crate::data::snapshot): the parse
//! happens once for the whole suite and every later process decodes the result
//! (627 ms → 85 ms for `SkillData`).

use crate::data::snapshot;
use std::sync::LazyLock;

const DIST: &str = crate::data::DIST_GAME;

/// Defines a borrowing accessor, and — given a second name — its cloning
/// twin. Catalogues no test needs to own stay borrow-only, so they don't
/// have to carry a `Clone` impl just for the tests.
macro_rules! cached {
    ($borrow:ident, $ty:ty) => {
        /// The shared parse — for tests that only read.
        pub fn $borrow() -> &'static $ty {
            static CELL: LazyLock<$ty> = LazyLock::new(|| {
                snapshot::cached(stringify!($borrow), DIST, || <$ty>::load_from(DIST))
            });
            &CELL
        }
    };
    ($borrow:ident, $owned:ident, $ty:ty) => {
        cached!($borrow, $ty);

        /// A private copy — for tests that mutate it or move it into a world.
        pub fn $owned() -> $ty {
            $borrow().clone()
        }
    };
}

cached!(game_data, game_data_owned, crate::data::GameData);
cached!(items, items_owned, crate::data::ItemData);
cached!(skills, skills_owned, crate::data::SkillData);
cached!(npcs, npcs_owned, crate::data::NpcData);
cached!(pets, pets_owned, crate::data::pet_data::PetData);
cached!(skill_trees, skill_trees_owned, crate::data::SkillTreeData);
cached!(
    player_templates,
    player_templates_owned,
    crate::data::PlayerTemplateData
);
cached!(spawns, crate::data::spawn_data::SpawnData);
cached!(karma, crate::data::karma_data::KarmaData);
