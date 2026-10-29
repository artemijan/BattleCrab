//! The `InstanceScript` / `Instance` primitives a script reaches through its
//! [`QuestCtx`]: entering, the world's status and parameters, its spawns and
//! doors, and finishing it. Thin wrappers over [`crate::game_loop::space::instances`].

use super::QuestCtx;
use crate::game_loop::helpers::{instance_of, is_dead};
use crate::game_loop::npc::npc_id_of;
use crate::game_loop::space::instances;

impl QuestCtx<'_> {
    /// `npc.getInstanceWorld()`'s id, 0 outside any instance. A deleted NPC
    /// keeps its `_instance` in Java, so one that has left the world still
    /// answers with the live instance it was spawned into (NPC object ids
    /// are never reused).
    pub fn npc_instance(&self) -> i32 {
        if self
            .world
            .objects
            .has_component::<crate::model::npc::Npc>(&self.npc)
        {
            return instance_of(self.world, self.npc);
        }
        self.world
            .instances
            .iter()
            .find(|(_, inst)| inst.npcs.contains(&self.npc))
            .map_or(0, |(id, _)| id)
    }

    /// `player.getInstanceWorld()`'s id, 0 outside any instance.
    pub fn player_instance(&self) -> i32 {
        instance_of(self.world, self.player)
    }

    /// `InstanceScript.enterInstance(player, npc, templateId)`.
    pub fn enter_instance(&mut self, template_id: i32) -> Option<i32> {
        if self.simulated {
            return None;
        }
        instances::enter_instance(self.world, self.player, template_id)
    }

    /// `Instance.getStatus`.
    pub fn instance_status(&self, instance_id: i32) -> i32 {
        self.world.instances.status(instance_id)
    }

    /// `Instance.setStatus`.
    pub fn set_instance_status(&mut self, instance_id: i32, status: i32) {
        self.world.instances.set_status(instance_id, status);
    }

    /// `Instance.getParameters().getInt(key, 0)`.
    pub fn instance_param(&self, instance_id: i32, key: &str) -> i64 {
        self.world.instances.get_var(instance_id, key)
    }

    /// `Instance.setParameter(key, value)`.
    pub fn set_instance_param(&mut self, instance_id: i32, key: &str, value: i64) {
        self.world.instances.set_var(instance_id, key, value);
    }

    /// `Instance.spawnGroup(name)`.
    pub fn spawn_instance_group(&mut self, instance_id: i32, name: &str) -> Vec<i32> {
        instances::spawn_group(self.world, instance_id, name)
    }

    /// `addSpawn(npcId, loc, false, 0, false, instanceId)`.
    pub fn spawn_in_instance(
        &mut self,
        instance_id: i32,
        npc_id: i32,
        (x, y, z, heading): (i32, i32, i32, i32),
    ) -> Option<i32> {
        instances::spawn_npc(self.world, instance_id, npc_id, x, y, z, heading)
    }

    /// `Instance.openCloseDoor(doorId, open)`.
    pub fn open_close_instance_door(&mut self, instance_id: i32, door_id: i32, open: bool) {
        instances::open_close_door(self.world, instance_id, door_id, open);
    }

    /// `Instance.finishInstance(delay)`.
    pub fn finish_instance(&mut self, instance_id: i32, delay_min: i32) {
        instances::finish_instance(self.world, instance_id, delay_min);
    }

    /// `Instance.getNpc(id)`: the first of the world's NPCs with that id.
    pub fn instance_npc(&self, instance_id: i32, npc_id: i32) -> Option<i32> {
        self.instance_npcs(instance_id, npc_id).into_iter().next()
    }

    /// `Instance.getAliveNpcs(id)`.
    pub fn instance_alive_npcs(&self, instance_id: i32, npc_id: i32) -> Vec<i32> {
        self.instance_npcs(instance_id, npc_id)
            .into_iter()
            .filter(|&oid| !is_dead(self.world, oid))
            .collect()
    }

    /// The world's NPCs with `npc_id` that still exist, in spawn order.
    fn instance_npcs(&self, instance_id: i32, npc_id: i32) -> Vec<i32> {
        self.world
            .instances
            .get(instance_id)
            .map(|i| {
                i.npcs
                    .iter()
                    .copied()
                    .filter(|&oid| npc_id_of(self.world, oid) == Some(npc_id))
                    .collect()
            })
            .unwrap_or_default()
    }
}
