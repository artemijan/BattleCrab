//! The ECS object store — stage 2 of the `bevy_ecs` adoption
//! (PLAN_ECS_STAGE2 §4). One `bevy_ecs::World` holds **all** world objects
//! (players and NPCs alike) as entities distinguished by their components
//! (`Player`/`Npc` residual cores double as the kind markers), plus one
//! object-id → [`Entity`] index (Java's `World._allObjects` role — the id
//! ranges are disjoint: NPCs allocate from `FIRST_NPC_OBJECT_ID` up,
//! persistent ids from the DB counter up).
//!
//! - Components live in bevy's archetype tables, so the hot per-tick sweeps
//!   (`for_each_mut` in the movement/AI/stance systems) are dense,
//!   cache-friendly table iteration; presence-based components (`Movement`,
//!   `Casting`, `Intent`) make those sweeps visit only the entities that
//!   carry them.
//! - By-id access stays O(1) through the index (`id → Entity → table row`).
//! - The single-owner model from `CONCURRENCY_MODEL.md` is unchanged: the
//!   store lives inside `World`, mutated by the game thread alone; object
//!   ids (`i32`) remain the only key the game logic speaks — `Entity` never
//!   leaves this module's API surface.
use std::collections::HashMap;

use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::{Component, Mutable};
use bevy_ecs::entity::Entity;
use bevy_ecs::query::{IterQueryData, QueryState, ReleaseStateQueryData, SingleEntityQueryData};
use bevy_ecs::world::{Mut, World as EcsWorld};

/// The unified object registry (players + NPCs), addressed by game object id.
#[derive(Default)]
pub struct EntityStore {
    ecs: EcsWorld,
    /// Game object id → ECS entity.
    index: HashMap<i32, Entity>,
}

impl EntityStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a new object from its full component bundle. On id reuse the
    /// whole entity is replaced (no live call site reuses ids).
    pub fn spawn(&mut self, id: i32, bundle: impl Bundle) {
        if let Some(entity) = self.index.remove(&id) {
            self.ecs.despawn(entity);
        }
        let entity = self.ecs.spawn(bundle).id();
        self.index.insert(id, entity);
    }

    /// Remove the object and every component with it. Callers that need
    /// state for a last rite (persistence snapshot, respawn bookkeeping)
    /// must gather it **before** despawning — components drop here.
    pub fn despawn(&mut self, id: &i32) -> bool {
        match self.index.remove(id) {
            Some(entity) => {
                self.ecs.despawn(entity);
                true
            }
            None => false,
        }
    }

    /// Whether any object (either kind) lives under this id.
    pub fn contains_id(&self, id: &i32) -> bool {
        self.index.contains_key(id)
    }

    pub fn get_component<C: Component>(&self, id: &i32) -> Option<&C> {
        self.index.get(id).and_then(|e| self.ecs.get::<C>(*e))
    }

    pub fn get_component_mut<C: Component<Mutability = Mutable>>(
        &mut self,
        id: &i32,
    ) -> Option<&mut C> {
        let entity = *self.index.get(id)?;
        self.ecs.get_mut::<C>(entity).map(Mut::into_inner)
    }

    /// `contains_id` narrowed by kind/component: `has_component::<Player>`
    /// is the "is this id a player?" test.
    pub fn has_component<C: Component>(&self, id: &i32) -> bool {
        self.get_component::<C>(id).is_some()
    }

    /// Borrow several components of one object at once, mutably-disjoint by
    /// type: `store.get_many_mut::<(&mut Position, &Player)>(&id)`. `None`
    /// when the id is dead or the entity lacks one of the requested
    /// components. NOTE: `Option<&C>` in `Q` errors (instead of matching
    /// as absent) while `C` was never registered — prefer a separate
    /// `has_component` probe over optional fetches here.
    pub fn get_many_mut<Q>(&mut self, id: &i32) -> Option<Q::Item<'_, 'static>>
    where
        Q: ReleaseStateQueryData + SingleEntityQueryData,
    {
        let entity = *self.index.get(id)?;
        self.ecs.entity_mut(entity).into_components_mut::<Q>().ok()
    }

    /// Add (or replace) components on an existing object — the insert half
    /// of presence-based state (`Movement`/`Casting`/`Intent`): presence is
    /// the sweep filter, so "start moving" is a component insert. No-op on a
    /// dead id.
    pub fn add_components(&mut self, id: &i32, bundle: impl Bundle) {
        if let Some(&entity) = self.index.get(id) {
            self.ecs.entity_mut(entity).insert(bundle);
        }
    }

    /// Remove a presence-based component, returning it (`None` when absent
    /// or the id is dead) — the "stop moving"/"cast over" half.
    pub fn remove_component<C: Component>(&mut self, id: &i32) -> Option<C> {
        let entity = *self.index.get(id)?;
        self.ecs.entity_mut(entity).take::<C>()
    }

    /// Sweep every object that has all of `Q`'s components — the per-tick
    /// system shape (`store.for_each_mut::<(&Movement, &mut Position)>(…)`).
    /// Builds the `QueryState` per call: construction cost scales with the
    /// archetype count (a handful), not the entity count, so this stays
    /// cheap even with the 34.9k-NPC world.
    pub fn for_each_mut<Q: IterQueryData>(&mut self, f: impl FnMut(Q::Item<'_, '_>)) {
        let mut query = QueryState::<Q>::new(&mut self.ecs);
        query.iter_mut(&mut self.ecs).for_each(f);
    }

    /// Objects carrying component `C` (e.g. `count::<Player>()` = players
    /// online). Same per-call `QueryState` note as `for_each_mut`.
    pub fn count<C: Component>(&mut self) -> usize {
        let mut query = QueryState::<&C>::new(&mut self.ecs);
        query.iter(&self.ecs).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component, Debug, PartialEq)]
    struct Thing(i32);

    #[derive(Component, Debug, PartialEq, Clone, Copy)]
    struct Extra(i32);

    #[test]
    fn spawn_get_despawn_roundtrip() {
        let mut s = EntityStore::new();
        s.spawn(7, (Thing(70), Extra(700)));
        s.spawn(8, Thing(80)); // no Extra on this one
        assert!(s.contains_id(&7) && s.contains_id(&8) && !s.contains_id(&9));
        assert_eq!(s.count::<Thing>(), 2);
        assert_eq!(s.count::<Extra>(), 1);
        assert_eq!(s.get_component::<Extra>(&7), Some(&Extra(700)));
        assert_eq!(s.get_component::<Extra>(&8), None);
        assert_eq!(s.get_component::<Thing>(&7), Some(&Thing(70)));

        s.get_component_mut::<Extra>(&7).unwrap().0 = 701;
        assert_eq!(s.get_component::<Extra>(&7), Some(&Extra(701)));

        // Multi-component disjoint borrow on one entity.
        let (mut t, mut e) = s.get_many_mut::<(&mut Thing, &mut Extra)>(&7).unwrap();
        t.0 += 1;
        e.0 += 1;
        assert_eq!(s.get_component::<Thing>(&7), Some(&Thing(71)));
        assert_eq!(s.get_component::<Extra>(&7), Some(&Extra(702)));
        // Missing component ⇒ None, dead id ⇒ None.
        assert!(s.get_many_mut::<(&mut Thing, &mut Extra)>(&8).is_none());
        assert!(s.get_many_mut::<(&mut Thing, &mut Extra)>(&9).is_none());

        // Tuple sweep visits only entities that have the full component set.
        let mut seen = Vec::new();
        s.for_each_mut::<(&Thing, &mut Extra)>(|(t, mut e)| {
            e.0 += 1;
            seen.push(t.0);
        });
        assert_eq!(seen, vec![71]);
        assert_eq!(s.get_component::<Extra>(&7), Some(&Extra(703)));

        // Presence toggling.
        s.remove_component::<Extra>(&7);
        assert!(!s.has_component::<Extra>(&7));
        s.add_components(&7, Extra(1));
        assert_eq!(s.get_component::<Extra>(&7), Some(&Extra(1)));

        // Despawn drops everything.
        assert!(s.despawn(&7));
        assert!(!s.despawn(&7));
        assert_eq!(s.get_component::<Thing>(&7), None);
        assert_eq!(s.count::<Thing>(), 1);
    }
}
