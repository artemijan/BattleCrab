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

    /// `Instance.getTemplateId()`, 0 for no live instance.
    pub fn instance_template(&self, instance_id: i32) -> i32 {
        self.world
            .instances
            .get(instance_id)
            .map_or(0, |i| i.template_id)
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

    /// `Instance.getAliveNpcs()`: every living NPC of the world.
    pub fn instance_all_alive_npcs(&self, instance_id: i32) -> Vec<i32> {
        self.world
            .instances
            .get(instance_id)
            .map(|i| {
                i.npcs
                    .iter()
                    .copied()
                    .filter(|oid| {
                        self.world
                            .objects
                            .has_component::<crate::model::npc::Npc>(oid)
                            && !is_dead(self.world, *oid)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `Instance.getAliveNpcs(Monster.class)`: the world's living monsters.
    pub fn instance_alive_monsters(&self, instance_id: i32) -> Vec<i32> {
        self.instance_all_alive_npcs(instance_id)
            .into_iter()
            .filter(|&oid| {
                crate::game_loop::npc::npc_template(self.world, oid).is_some_and(|t| t.is_monster())
            })
            .collect()
    }

    /// `Instance.despawnGroup(name)`: delete every NPC the named group
    /// spawned.
    pub fn despawn_instance_group(&mut self, instance_id: i32, name: &str) {
        if self.simulated {
            return;
        }
        instances::despawn_group(self.world, instance_id, name);
    }

    /// `instance.broadcastPacket(ExShowUsm.X)`: every member plays USM
    /// video `video_id`.
    pub fn instance_show_usm(&self, instance_id: i32, video_id: i32) {
        instances::broadcast_to_instance(
            self.world,
            instance_id,
            &crate::network::server_packets::ex_show_usm(video_id),
        );
    }

    /// `Instance.removeNpcs()`.
    pub fn remove_instance_npcs(&mut self, instance_id: i32) {
        if self.simulated {
            return;
        }
        instances::remove_npcs(self.world, instance_id);
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

    /// `Instance.getFirstPlayer()`: the world's player, for the solo
    /// instances that call it. Several members resolve to the lowest id.
    pub fn instance_first_player(&self, instance_id: i32) -> Option<i32> {
        self.world
            .instances
            .get(instance_id)
            .and_then(|i| i.members.keys().min().copied())
    }

    /// `InstanceScript.showOnScreenMsg(instance, npcStringId, position, time,
    /// showEffect)`: the banner to every player in the world.
    pub fn instance_screen_message_npc_string(
        &self,
        instance_id: i32,
        npc_string_id: i32,
        position: i32,
        time: i32,
        show_effect: bool,
    ) {
        if self.simulated {
            return;
        }
        let pkt = crate::network::server_packets::ex_show_screen_message_npc_string_effect(
            npc_string_id,
            position,
            time,
            show_effect,
            &[],
        );
        instances::broadcast_to_instance(self.world, instance_id, &pkt);
    }

    /// `instance.broadcastPacket(new Earthquake(creature, intensity,
    /// duration))`.
    pub fn instance_earthquake(&self, instance_id: i32, at: i32, intensity: i32, duration: i32) {
        if self.simulated {
            return;
        }
        let Some(p) = crate::game_loop::space::position::maybe_position(self.world, at) else {
            return;
        };
        let pkt = crate::network::server_packets::earthquake(p.x, p.y, p.z, intensity, duration);
        instances::broadcast_to_instance(self.world, instance_id, &pkt);
    }

    /// The NPC half of `World.getVisibleObjects(object, …)` and, with a
    /// range, `getVisibleObjectsInRange`: the NPCs in the 3×3 regions around
    /// `oid`, in its instance, other than itself, within `range` in 3D when
    /// one is given. The caller applies the class filter.
    pub fn visible_npcs(&self, oid: i32, range: Option<i32>) -> Vec<i32> {
        let Some(region) = crate::game_loop::space::position::region_cell_of(self.world, oid)
        else {
            return Vec::new();
        };
        let Some(origin) = crate::game_loop::space::position::maybe_position(self.world, oid)
        else {
            return Vec::new();
        };
        let instance = instance_of(self.world, oid);
        self.world
            .npcs_visible_from(region)
            .into_iter()
            .filter(|&n| n != oid && instance_of(self.world, n) == instance)
            .filter(|&n| {
                let Some(r) = range else { return true };
                crate::game_loop::space::position::maybe_position(self.world, n).is_some_and(|p| {
                    let (dx, dy, dz) = (
                        f64::from(p.x - origin.x),
                        f64::from(p.y - origin.y),
                        f64::from(p.z - origin.z),
                    );
                    (dx * dx + dy * dy + dz * dz).sqrt() <= f64::from(r)
                })
            })
            .collect()
    }

    /// `npc instanceof FriendlyNpc`.
    pub fn npc_is_friendly(&self, npc: i32) -> bool {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc)
            .is_some_and(|n| n.friendly_ai(self.world))
    }

    /// `npc instanceof Attackable`.
    pub fn npc_is_attackable_class(&self, npc: i32) -> bool {
        crate::game_loop::npc::npc_template(self.world, npc)
            .is_some_and(|t| t.is_attackable_class())
    }
}
