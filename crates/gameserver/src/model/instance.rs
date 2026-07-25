//! Instances (G27) — logical world partitions (Java `instancemanager/
//! InstanceManager` + `model/instancezone/Instance`). An instance is a private
//! copy of some region: objects in it interact only with each other, not with
//! the overworld (instance 0) or other instances.

use std::collections::HashMap;

/// A live instance's bookkeeping (Java `Instance`). Slice 1 tracks only the
/// template it was created from; spawns/doors/exit come with the template
/// loader in a later slice.
#[derive(Debug, Clone)]
pub struct Instance {
    /// The `InstanceTemplate` id, or 0 for a bare instance (an Olympiad arena).
    pub template_id: i32,
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
        self.live.insert(id, Instance { template_id });
        id
    }

    /// Tear an instance down (Java `Instance.destroy`).
    pub fn destroy(&mut self, id: i32) {
        self.live.remove(&id);
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
