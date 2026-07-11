//! ECS-backed object registries — stage 1 of the `bevy_ecs` adoption.
//!
//! `World.players` / `World.npcs` used to be plain `HashMap<i32, T>`s. Each is
//! now an [`EntityStore<T>`]: a dedicated `bevy_ecs::World` whose entities
//! carry the game object as a single (fat) component, plus an
//! object-id → [`Entity`] index. The store keeps the HashMap-shaped API the
//! game-loop handlers were written against, so the single-owner model from
//! `CONCURRENCY_MODEL.md` is unchanged — only the storage engine moved:
//!
//! - Components live in bevy's archetype tables, so the hot per-tick sweeps
//!   (`values_mut` in regen/movement/AI systems) are dense, cache-friendly
//!   table iteration through a cached [`QueryState`] instead of a HashMap
//!   bucket walk.
//! - By-id access stays O(1) through the index (`id → Entity → table row`).
//! - Keeping players and NPCs in *separate* ECS worlds preserves the
//!   disjoint-field borrows the handlers rely on (`&mut` a player while
//!   reading `world.npcs`). Splitting the fat components into shared ones
//!   (`Position`, `Vitals`, …) and merging into one world is the follow-up
//!   stage — see `CONCURRENCY_MODEL.md` §2.8.
use std::collections::HashMap;

use bevy_ecs::component::{Component, Mutable};
use bevy_ecs::entity::Entity;
use bevy_ecs::query::QueryState;
use bevy_ecs::world::{Mut, World as EcsWorld};

/// One object registry (players or NPCs) stored in a `bevy_ecs` world,
/// addressed by game object id.
pub struct EntityStore<T: Component<Mutability = Mutable>> {
    ecs: EcsWorld,
    /// Game object id → ECS entity (Java's `World._allObjects` role).
    index: HashMap<i32, Entity>,
    /// Cached query for dense `values_mut` iteration (bevy re-validates it
    /// against new archetypes on each use).
    query: QueryState<&'static mut T>,
}

impl<T: Component<Mutability = Mutable>> EntityStore<T> {
    pub fn new() -> Self {
        let mut ecs = EcsWorld::new();
        let query = QueryState::new(&mut ecs);
        Self { ecs, index: HashMap::new(), query }
    }

    pub fn get(&self, id: &i32) -> Option<&T> {
        self.index.get(id).and_then(|e| self.ecs.get::<T>(*e))
    }

    pub fn get_mut(&mut self, id: &i32) -> Option<&mut T> {
        let entity = *self.index.get(id)?;
        self.ecs.get_mut::<T>(entity).map(Mut::into_inner)
    }

    /// HashMap-style insert: spawns an entity for a new id, replaces the
    /// component (returning the old value) for an existing one.
    pub fn insert(&mut self, id: i32, value: T) -> Option<T> {
        match self.index.get(&id) {
            Some(&entity) => {
                let mut e = self.ecs.entity_mut(entity);
                let old = e.take::<T>();
                e.insert(value);
                old
            }
            None => {
                let entity = self.ecs.spawn(value).id();
                self.index.insert(id, entity);
                None
            }
        }
    }

    pub fn remove(&mut self, id: &i32) -> Option<T> {
        let entity = self.index.remove(id)?;
        let value = self.ecs.entity_mut(entity).take::<T>();
        self.ecs.despawn(entity);
        value
    }

    pub fn contains_key(&self, id: &i32) -> bool {
        self.index.contains_key(id)
    }

    pub fn keys(&self) -> impl Iterator<Item = &i32> + '_ {
        self.index.keys()
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> + '_ {
        self.index.values().filter_map(|e| self.ecs.get::<T>(*e))
    }

    /// Dense table iteration over every object — the per-tick system sweep.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> + '_ {
        self.query.iter_mut(&mut self.ecs).map(Mut::into_inner)
    }
}

impl<T: Component<Mutability = Mutable>> std::ops::Index<&i32> for EntityStore<T> {
    type Output = T;

    /// HashMap-style `store[&id]`; panics on a missing id, like `HashMap`.
    fn index(&self, id: &i32) -> &T {
        self.get(id).expect("no object for id")
    }
}

impl<T: Component<Mutability = Mutable>> Default for EntityStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component, Debug, PartialEq)]
    struct Thing(i32);

    #[test]
    fn insert_get_remove_roundtrip() {
        let mut s = EntityStore::<Thing>::new();
        assert!(s.is_empty());
        assert_eq!(s.insert(7, Thing(70)), None);
        assert_eq!(s.insert(8, Thing(80)), None);
        assert_eq!(s.len(), 2);
        assert!(s.contains_key(&7));
        assert_eq!(s.get(&7), Some(&Thing(70)));
        s.get_mut(&7).unwrap().0 = 71;
        assert_eq!(s.get(&7), Some(&Thing(71)));
        // HashMap-style overwrite returns the old value.
        assert_eq!(s.insert(8, Thing(88)), Some(Thing(80)));
        assert_eq!(s.remove(&8), Some(Thing(88)));
        assert_eq!(s.get(&8), None);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn iteration_sees_all_live_objects() {
        let mut s = EntityStore::<Thing>::new();
        for id in 0..5 {
            s.insert(id, Thing(id));
        }
        s.remove(&3);
        let mut seen: Vec<i32> = s.values().map(|t| t.0).collect();
        seen.sort();
        assert_eq!(seen, vec![0, 1, 2, 4]);
        for t in s.values_mut() {
            t.0 += 100;
        }
        let mut seen: Vec<i32> = s.values().map(|t| t.0).collect();
        seen.sort();
        assert_eq!(seen, vec![100, 101, 102, 104]);
        let mut keys: Vec<i32> = s.keys().copied().collect();
        keys.sort();
        assert_eq!(keys, vec![0, 1, 2, 4]);
    }
}
