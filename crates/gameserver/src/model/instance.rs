//! Instances (G27) — logical world partitions (Java `instancemanager/
//! InstanceManager` + `model/instancezone/Instance`). An instance is a private
//! copy of some region: objects in it interact only with each other, not with
//! the overworld (instance 0) or other instances.

use std::collections::HashMap;

/// A live instance's bookkeeping (Java `Instance`).
#[derive(Debug, Clone, Default)]
pub struct Instance {
    /// The `InstanceTemplate` id, or 0 for a bare instance (an Olympiad arena).
    pub template_id: i32,
    /// Object ids of the NPCs spawned into this instance, torn down with it.
    pub npcs: Vec<i32>,
    /// Members currently inside, and where each is returned to on an ORIGIN exit.
    pub members: HashMap<i32, (i32, i32, i32)>,
    /// The game tick at which the instance emptied, for the empty-destroy timer.
    pub empty_since: Option<u64>,
}

/// Allocates instance ids and tracks the live instances (Java `InstanceManager`).
#[derive(Debug, Default)]
pub struct InstanceManager {
    next_id: i32,
    live: HashMap<i32, Instance>,
}

impl InstanceManager {
    /// Create a new instance and return its id — always ≥ 1, since 0 is the
    /// shared overworld.
    pub fn create(&mut self, template_id: i32) -> i32 {
        self.next_id += 1;
        let id = self.next_id;
        self.live.insert(
            id,
            Instance {
                template_id,
                ..Default::default()
            },
        );
        id
    }

    /// Tear an instance down (Java `Instance.destroy`).
    pub fn destroy(&mut self, id: i32) {
        self.live.remove(&id);
    }

    pub fn get(&self, id: i32) -> Option<&Instance> {
        self.live.get(&id)
    }

    /// Record an NPC spawned into the instance (for teardown).
    pub fn record_npc(&mut self, id: i32, npc_oid: i32) {
        if let Some(inst) = self.live.get_mut(&id) {
            inst.npcs.push(npc_oid);
        }
    }

    /// Add a member, remembering where to return them on an ORIGIN exit.
    pub fn add_member(&mut self, id: i32, player: i32, return_to: (i32, i32, i32)) {
        if let Some(inst) = self.live.get_mut(&id) {
            inst.members.insert(player, return_to);
            inst.empty_since = None;
        }
    }

    /// Remove a member; returns their stored return location. Stamps
    /// `empty_since` (at `now_tick`) when the instance becomes empty.
    pub fn remove_member(
        &mut self,
        id: i32,
        player: i32,
        now_tick: u64,
    ) -> Option<(i32, i32, i32)> {
        let inst = self.live.get_mut(&id)?;
        let ret = inst.members.remove(&player);
        if inst.members.is_empty() {
            inst.empty_since = Some(now_tick);
        }
        ret
    }

    pub fn member_count(&self, id: i32) -> usize {
        self.live.get(&id).map_or(0, |i| i.members.len())
    }

    pub fn contains(&self, id: i32) -> bool {
        self.live.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
}
