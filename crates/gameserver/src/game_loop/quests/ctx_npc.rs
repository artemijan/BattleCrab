//! `QuestCtx` NPC manipulation: spawn/despawn, casts, soul-crystal
//! absorbing, retargeting, NPC variables/say and quest timers.

use super::QuestCtx;
use super::load_quest_html;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::client_for_player;
use crate::game_loop::helpers::hp_fraction;
use crate::game_loop::helpers::pos_of;
use crate::game_loop::npc::cast;
use crate::model::components::QuestTimerSeqs;
use crate::model::components::Quests;
use crate::model::inventory::Inventory;
use crate::model::quest::state;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::scheduler::ms_to_ticks;
use crate::world::World;

impl<'w> QuestCtx<'w> {
    /// `Attackable.isSpoiled()` — whether a Spoil landed on the involved NPC.
    /// Quest 417 (the Scavenger path) pays out only on spoiled corpses.
    pub fn npc_is_spoiled(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .is_some_and(|n| n.spoiler_object_id != 0)
    }

    /// `Attackable.getSpoilerObjectId()`.
    pub fn npc_spoiler_object_id(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .map(|n| n.spoiler_object_id)
            .unwrap_or(0)
    }

    /// `npc.deleteMe()` — remove the involved NPC from the world.
    pub fn delete_npc(&mut self) {
        if self.simulated {
            return;
        }
        let Some(region) = self
            .world
            .objects
            .get_component::<crate::model::components::RegionCell>(&self.npc)
            .map(|r| r.0)
        else {
            return;
        };
        crate::game_loop::death::despawn_npc(self.world, self.npc, region);
    }

    /// `addSpawn(npcId, npc, randomOffset, 0, …)` followed by
    /// `addAttackPlayerDesire(spawned, player)` — the quest-ambush primitive.
    /// Spawns beside the NPC the player is talking to and sets the newcomer on
    /// them.
    ///
    /// `random_offset` reproduces Java's `Rnd.get(50, 100)` per axis with an
    /// independent sign, so a group of ambushers doesn't stack on one point.
    pub fn spawn_attacker(&mut self, npc_id: i32, random_offset: bool) -> Option<i32> {
        let target = self.player;
        self.spawn_attacker_on(npc_id, random_offset, target)
    }

    /// As [`spawn_attacker`](Self::spawn_attacker) but aims the newcomer at a
    /// specific playable — `addAttackPlayerDesire(spawned, attacker)` where
    /// Java picked the summon over its owner.
    pub fn spawn_attacker_on(
        &mut self,
        npc_id: i32,
        random_offset: bool,
        target_oid: i32,
    ) -> Option<i32> {
        let spawned = self.spawn_near_npc(npc_id, random_offset)?;
        crate::game_loop::ai::seed_attack(self.world, spawned, target_oid);
        Some(spawned)
    }

    /// `player.getAnyServitor()` — the acting player's summoned servitor, `None`
    /// if none is out. The counterpart to [`attack_is_summon`], for the
    /// servitor-duel quests (230) that pit a summon against a rival NPC.
    ///
    /// [`attack_is_summon`]: Self::attack_is_summon
    pub fn owner_servitor(&self) -> Option<i32> {
        crate::game_loop::servitor::servitor_of(self.world, self.player)
    }

    /// `player.getPet()` → its `getControlObjectId()`: the object id of the
    /// collar (a Dragonflute, in quest 421) that summoned the pet, or `None`
    /// when no *pet* is out. A servitor is not a pet — this is the
    /// item-summoned companion only, whose identity quest 421 binds its
    /// hatchling to (`summon.getControlObjectId() == fluteObjectId`).
    pub fn pet_control_object_id(&self) -> Option<i32> {
        let pet = crate::game_loop::servitor::pet_of(self.world, self.player)?;
        self.world
            .objects
            .get_component::<crate::model::components::PetOf>(&pet)
            .map(|p| p.collar_object_id)
    }

    /// Java's `isSummon ? killer.getServitors()…orElse(getPet()) : killer` —
    /// the **Playable** that actually landed the blow, which is what the
    /// ambush scripts set their spawn on. Falls back to the player when the
    /// summon has already gone (Java's `orElse` chain can yield null; a null
    /// target there simply means no desire is set).
    pub fn killing_playable(&self) -> i32 {
        if !self.attack_is_summon {
            return self.player;
        }
        crate::game_loop::servitor::servitor_of(self.world, self.player)
            .or_else(|| crate::game_loop::servitor::pet_of(self.world, self.player))
            .unwrap_or(self.player)
    }

    /// `player.hasSummon()` — a pet or a servitor is out.
    pub fn has_summon(&self) -> bool {
        crate::game_loop::servitor::pet_of(self.world, self.player).is_some()
            || crate::game_loop::servitor::servitor_of(self.world, self.player).is_some()
    }

    /// `player.getInventory().getItemByItemId(id).getObjectId()` — the object id
    /// of the first inventory item of `item_id`, `None` if absent.
    pub fn item_object_id(&self, item_id: i32) -> Option<i32> {
        self.world
            .objects
            .get_component::<Inventory>(&self.player)
            .and_then(|inv| inv.first_of_item(item_id).map(|i| i.object_id))
    }

    /// `getItemByItemId(id).getEnchantLevel()` — the enchant level of the first
    /// inventory item of `item_id`, `None` if absent. Quest 421 reads a
    /// Dragonflute's enchant level as its hatchling's level.
    pub fn item_enchant_level(&self, item_id: i32) -> Option<i32> {
        self.world
            .objects
            .get_component::<Inventory>(&self.player)
            .and_then(|inv| inv.first_of_item(item_id).map(|i| i.enchant_level))
    }

    /// `startQuestTimer("DESPAWN…", delay, npc, null)` for a **spawned** NPC:
    /// schedule its deletion `delay_ms` from now. Unlike [`start_quest_timer`],
    /// which fires `on_timer` carrying the in-context NPC, this deletes an
    /// arbitrary spawned actor with no script round-trip — quest 421's Soul of
    /// Tree Guardian ambush arms one per guardian.
    ///
    /// [`start_quest_timer`]: Self::start_quest_timer
    /// Java `Util.checkIfInRange(range, npc, player, includeZAxis)` for the
    /// quest ctx's own pair — the guard most `onKill` bodies open with, so a
    /// party member farming on the other side of the map does not collect.
    ///
    /// Java measures **centre to centre plus both collision radii**; the port
    /// has the radii on the templates but a quest kill is always npc↔player,
    /// where the difference is a few units against a 1500-unit range. Plain
    /// centre distance is used, and `include_z` matches Java's flag rather
    /// than always being on: several callers pass false.
    pub fn in_range_of_npc(&self, other_oid: i32, range: i32, include_z: bool) -> bool {
        let pos = |oid: i32| {
            self.world
                .objects
                .get_component::<crate::model::components::Position>(&oid)
                .map(|p| (p.x as f64, p.y as f64, p.z as f64))
        };
        let (Some(a), Some(b)) = (pos(self.npc), pos(other_oid)) else {
            // A dead or despawned actor is not "in range" — refusing is the
            // safe answer for a reward gate.
            return false;
        };
        let (dx, dy) = (a.0 - b.0, a.1 - b.1);
        let d2 = dx * dx + dy * dy + if include_z { (a.2 - b.2).powi(2) } else { 0.0 };
        d2 <= (range as f64).powi(2)
    }

    pub fn schedule_despawn(&mut self, npc_oid: i32, delay_ms: u64) {
        if self.simulated {
            return;
        }
        let fire_at = self.world.tick + ms_to_ticks(delay_ms);
        self.world
            .scheduler
            .schedule(fire_at, ScheduledTask::DespawnNpc { npc_oid });
    }

    /// `addAttackPlayerDesire(npc, target)` on the **in-context NPC** — send it
    /// after `target_oid` (a servitor, in quest 230's arcana duels), the mirror
    /// of [`spawn_attacker`](Self::spawn_attacker) which seeds a *spawned* NPC.
    pub fn make_npc_attack(&mut self, target_oid: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::ai::seed_attack(self.world, self.npc, target_oid);
    }

    /// `Attackable.addAbsorber(caster)`: record the acting player as an absorber
    /// of the in-context NPC, tagged with the NPC's HP **right now** (quest
    /// 350's Soul Crystal cast). A repeat cast overwrites, as in Java's map.
    pub fn add_absorber(&mut self) {
        if self.simulated {
            return;
        }
        let Some(hp) = self
            .world
            .objects
            .get_component::<crate::model::components::Vitals>(&self.npc)
            .map(|v| v.cur_hp)
        else {
            return;
        };
        let player = self.player;
        if let Some(a) = self
            .world
            .objects
            .get_component_mut::<crate::model::npc::Absorbers>(&self.npc)
        {
            a.0.insert(player, hp);
        } else {
            let mut a = crate::model::npc::Absorbers::default();
            a.0.insert(player, hp);
            self.world.objects.add_components(&self.npc, a);
        }
    }

    /// Java `levelSoulCrystals`' skill gate: the acting player is in the
    /// in-context NPC's absorber list **and** cast the crystal skill while the
    /// NPC was at ≤ half HP (`AbsorberInfo.getAbsorbedHp() <= maxHp/2`).
    pub fn killer_absorbed_below_half(&self) -> bool {
        let Some(max_hp) = self
            .world
            .objects
            .get_component::<crate::model::components::Vitals>(&self.npc)
            .map(|v| v.max_hp)
        else {
            return false;
        };
        self.world
            .objects
            .get_component::<crate::model::npc::Absorbers>(&self.npc)
            .and_then(|a| a.0.get(&self.player).copied())
            .is_some_and(|hp| hp <= max_hp as f64 / 2.0)
    }

    /// The Soul Crystal data table (`LevelUpCrystalData.xml`).
    pub fn soul_crystal_data(&self) -> &crate::data::SoulCrystalData {
        &self.world.data.soul_crystal_data
    }

    /// Java `getSCForPlayer`: the item id of the **single** Soul Crystal the
    /// acting player carries, or `None` if they hold none or more than one
    /// (an ambiguous inventory levels nothing).
    pub fn single_soul_crystal(&self) -> Option<i32> {
        let inv = self
            .world
            .objects
            .get_component::<Inventory>(&self.player)?;
        let scd = &self.world.data.soul_crystal_data;
        let mut found = None;
        for item in inv.items() {
            if scd.crystal(item.item_id).is_some() {
                if found.is_some() {
                    return None;
                }
                found = Some(item.item_id);
            }
        }
        found
    }

    /// `npc.broadcastSay(NPC_GENERAL, text)` — a literal-text chat bubble from
    /// the in-context NPC (e.g. the Saga finale boss's retreat cry).
    pub fn npc_say_text(&self, text: &str) {
        if self.simulated {
            return;
        }
        crate::game_loop::npc::say::npc_say_text(self.world, self.npc, text);
    }

    /// The same literal-text bubble, but from an *arbitrary* npc — a spawned
    /// finale actor rather than the in-context one.
    pub fn broadcast_npc_text(&self, npc_oid: i32, text: &str) {
        if self.simulated {
            return;
        }
        crate::game_loop::npc::say::npc_say_text(self.world, npc_oid, text);
    }

    /// Seed aggro from an arbitrary npc onto a target (npc-vs-npc), for the Saga
    /// finale where the companion and boss duel each other.
    pub fn seed_npc_attack(&mut self, npc_oid: i32, target_oid: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::ai::seed_attack(self.world, npc_oid, target_oid);
    }

    /// `npc.getCurrentHp() / npc.getMaxHp()` for the in-context NPC, as a
    /// fraction in `0.0..=1.0`. A missing or zero-max target reads as full
    /// health, so an HP *threshold* test fails closed rather than firing on
    /// every NPC the script cannot see.
    pub fn npc_hp_ratio(&self) -> f64 {
        hp_fraction(self.world, self.npc).unwrap_or(1.0)
    }

    /// `npc.setTarget(target); npc.doCast(skill)` — a **real** cast by an NPC,
    /// with the skill's effects, not just its animation.
    ///
    /// The counterpart to [`cast_visual_at`], and the difference is the whole
    /// point: `cast_visual_at` broadcasts a `MagicSkillUse` and nothing
    /// happens, while this routes through the same `npc_cast::start_cast` the
    /// boss AIs use, so the target really is rooted / poisoned / cursed. Reach
    /// for the visual one only where Java's call is a bare `broadcastPacket`.
    ///
    /// Returns whether the cast started. It will not when the skill id is
    /// absent from the datapack or `check_use_conditions` refuses — Java's
    /// `doCast` runs its own checks and quietly does nothing on failure, so a
    /// caller that ignores the result matches it.
    ///
    /// [`cast_visual_at`]: QuestCtx::cast_visual_at
    pub fn npc_cast(
        &mut self,
        caster_oid: i32,
        target_oid: i32,
        skill_id: i32,
        level: i32,
    ) -> bool {
        !self.simulated && cast::cast_skill(self.world, caster_oid, target_oid, skill_id, level)
    }

    /// `<caster>.broadcastPacket(new MagicSkillUse(caster, target, skillId,
    /// level, hitTime, reuse))` — one visual cast from `caster_oid` aimed at
    /// `target_oid`, shown to everyone nearby.
    ///
    /// Distinct from [`cast_visual`], which emits *two* self-casts (one on the
    /// player, one on the in-context NPC). Java's quest scripts mostly want
    /// this shape instead: quest 125's Ulu Kaimu casts at the player
    /// (`npc → player`), quest 235's mixing flash is the player on themselves
    /// (`player → player`). Passing the same oid twice gives the self-cast.
    ///
    /// `reuse` is not on the wire in this chronicle's packet, so it is not a
    /// parameter — Java's differing values (0 vs 1) make no observable
    /// difference here.
    ///
    /// [`cast_visual`]: QuestCtx::cast_visual
    pub fn cast_visual_at(
        &self,
        caster_oid: i32,
        target_oid: i32,
        skill_id: i32,
        level: i32,
        hit_time: i32,
    ) {
        if self.simulated {
            return;
        }
        let at = |oid: i32| {
            self.world
                .objects
                .get_component::<crate::model::components::Position>(&oid)
                .map(|p| (oid, p.x, p.y, p.z))
        };
        let (Some(caster), Some(target)) = (at(caster_oid), at(target_oid)) else {
            return;
        };
        let Some(region) = self
            .world
            .objects
            .get_component::<crate::model::components::RegionCell>(&caster_oid)
            .map(|r| r.0)
        else {
            return;
        };
        let pkt = server_packets::magic_skill_use_raw(caster, target, skill_id, level, hit_time);
        crate::game_loop::helpers::broadcast_near_region(self.world, region, &pkt);
    }

    /// L2J's `Cast(npc, player, skillId, level)` — a purely visual self-cast
    /// `MagicSkillUse` shown on **both** the in-context NPC and the player, to
    /// everyone nearby. The Saga rite uses it for the tablet progression glow
    /// (4546) and the final transform flash (4339).
    pub fn cast_visual(&self, skill_id: i32, level: i32) {
        if self.simulated {
            return;
        }
        for oid in [self.player, self.npc] {
            let pos = maybe_position(self.world, oid);
            let region = self
                .world
                .objects
                .get_component::<crate::model::components::RegionCell>(&oid)
                .map(|r| r.0);
            if let (Some(pos), Some(region)) = (pos, region) {
                let pkt = server_packets::magic_skill_use_raw(
                    (oid, pos.x, pos.y, pos.z),
                    (oid, pos.x, pos.y, pos.z),
                    skill_id,
                    level,
                    6000,
                );
                crate::game_loop::helpers::broadcast_near_region(self.world, region, &pkt);
            }
        }
    }

    /// Whether the object `oid` is dead (or gone). Used by the arcana-duel
    /// `KILLED_ATTACKER` timer to tell whether the challenger's servitor fell.
    pub fn is_oid_dead(&self, oid: i32) -> bool {
        self.world
            .objects
            .get_component::<crate::model::components::Vitals>(&oid)
            .is_none_or(|v| v.dead)
    }

    /// `addSpawn(npcId, x, y, z, …)` + `addAttackPlayerDesire` — a hostile spawn
    /// at a **fixed world position** rather than beside the in-context NPC (which
    /// is what [`spawn_attacker`](Self::spawn_attacker) does). Quest 231's
    /// teleport ambush conjures its King Bugbears at the arrival spot.
    pub fn spawn_attacker_at(&mut self, npc_id: i32, x: i32, y: i32, z: i32) -> Option<i32> {
        if self.simulated {
            return None;
        }
        let spawned = crate::game_loop::npc::spawn_npc_at(self.world, npc_id, x, y, z, -1)?;
        crate::game_loop::death::introduce_npc(self.world, spawned);
        self.link_summoned(spawned);
        crate::game_loop::ai::seed_attack(self.world, spawned, self.player);
        Some(spawned)
    }

    /// `addSpawn(npcId, npc, randomOffset, 0, …)` **without**
    /// `addAttackPlayerDesire` — the newcomer appears and is left alone.
    /// Quest 416 spawns its Durka Spirit this way: Java conjures it beside the
    /// dead spider and does *not* set it on the player, unlike quest 414's
    /// Kuruka. Keep the two apart; aggroing here would be an invention.
    pub fn spawn_near_npc(&mut self, npc_id: i32, random_offset: bool) -> Option<i32> {
        if self.simulated {
            return None;
        }
        let (mut x, mut y, z) = pos_of(self.world, self.npc)?;
        if random_offset {
            for axis in [&mut x, &mut y] {
                let offset = self.world.roll(51) + 50; // Rnd.get(50, 100)
                let sign = if self.world.roll(2) == 0 { -1 } else { 1 };
                *axis += offset * sign;
            }
        }
        let spawned = crate::game_loop::npc::spawn_npc_at(self.world, npc_id, x, y, z, -1)?;
        crate::game_loop::death::introduce_npc(self.world, spawned);
        self.link_summoned(spawned);
        Some(spawned)
    }

    /// Java `getRandomPartyMemberState(player, condition, playerChance, npc)`
    /// / `getRandomPartyMember(player, cond)` — re-target this ctx at a random
    /// qualifying party member and return whether one was found:
    /// `condition == -1` means "any STARTED state", otherwise the member must
    /// be exactly on that cond; the original player is weighted
    /// `player_chance`× (retail kill credit favours the killer 2-3×); and the
    /// pick must stand within `AltPartyRange` of the involved NPC. Solo, the
    /// player is the only candidate. On success `ctx.player`/`ctx.client_id`
    /// point at the pick, so every later give/set lands on them, like Java's
    /// returned `QuestState`.
    pub fn retarget_random_party_member(&mut self, condition: i32, player_chance: i32) -> bool {
        let name = self.script.name();
        let qualifies = |world: &World, oid: i32| {
            world
                .objects
                .get_component::<Quests>(&oid)
                .and_then(|q| q.0.get(name))
                .is_some_and(|qs| {
                    if condition == -1 {
                        qs.state == state::STARTED
                    } else {
                        qs.state == state::STARTED && qs.cond() == condition
                    }
                })
        };
        let killer = self.player;
        let members: Vec<i32> =
            crate::game_loop::party::party_members(self.world, killer).unwrap_or_default();
        let in_range = |world: &World, oid: i32, npc: i32| {
            if npc == 0 {
                return true;
            }
            crate::geo::distance::within_3d(
                world,
                oid,
                npc,
                f64::from(world.cfg.character.alt_party_range),
            )
        };
        if members.is_empty() {
            // Java's solo arm range-checks too.
            return qualifies(self.world, killer) && in_range(self.world, killer, self.npc);
        }
        let mut candidates: Vec<i32> = Vec::new();
        if qualifies(self.world, killer) {
            for _ in 0..player_chance.max(1) {
                candidates.push(killer);
            }
        }
        for &m in &members {
            if m != killer && qualifies(self.world, m) {
                candidates.push(m);
            }
        }
        if candidates.is_empty() {
            return false;
        }
        let pick = candidates[self.world.roll(candidates.len() as i32) as usize];
        // `checkDistanceToTarget`: within `AltPartyRange` (3D) of the NPC.
        if !in_range(self.world, pick, self.npc) {
            return false;
        }
        self.player = pick;
        self.client_id = client_for_player(self.world, pick).unwrap_or(0);
        true
    }

    /// `addSpawn(npcId, x, y, z, heading, false, 0)` **without** any attack
    /// desire — a fixed-spot bystander. Quest 227's staged duels spawn the
    /// decoy Ol Mahum Pilgrim (and its attacker) this way; the aggro is
    /// seeded separately, at the decoy, not the player.
    pub fn spawn_bystander_at(&mut self, npc_id: i32, x: i32, y: i32, z: i32) -> Option<i32> {
        if self.simulated {
            return None;
        }
        let spawned = crate::game_loop::npc::spawn_npc_at(self.world, npc_id, x, y, z, 0)?;
        crate::game_loop::death::introduce_npc(self.world, spawned);
        self.link_summoned(spawned);
        Some(spawned)
    }

    /// `summoner.addSummonedNpc(npc)` — record that the *talking* NPC spawned
    /// this one, so [`summoned_npc_count`](Self::summoned_npc_count) can cap
    /// repeat spawns.
    ///
    /// [`Self::summoned_npc_count`]: Self::summoned_npc_count
    fn link_summoned(&mut self, spawned: i32) {
        use crate::model::components::SummonedNpcs;
        let npc = self.npc;
        match self.world.objects.get_component_mut::<SummonedNpcs>(&npc) {
            Some(list) => list.0.push(spawned),
            None => self
                .world
                .objects
                .add_components(&npc, SummonedNpcs(vec![spawned])),
        }
    }

    /// Java `npc.getSummonedNpcCount()` — how many of the NPCs this one spawned
    /// are still in the world. Scripts gate re-spawns on it (`< 1`, `< 5`,
    /// `< 10`, …) so a repeatedly-clicked dialog doesn't flood the map.
    ///
    /// Dead-but-undecayed children still count: Java unlinks at `onDecay`, and
    /// a corpse is still an object here too.
    pub fn summoned_npc_count(&self) -> usize {
        use crate::model::components::SummonedNpcs;
        self.world
            .objects
            .get_component::<SummonedNpcs>(&self.npc)
            .map(|l| {
                l.0.iter()
                    .filter(|oid| {
                        self.world
                            .objects
                            .has_component::<crate::model::npc::Npc>(oid)
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// `npc.getVariables().getInt(key)` — 0 when unset, as in Java.
    pub fn npc_var_int(&self, key: &str) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .and_then(|n| n.vars.get(key).copied())
            .unwrap_or(0)
    }

    /// `npc.getVariables().set(key, value)`.
    pub fn set_npc_var_int(&mut self, key: &str, value: i32) {
        if self.simulated {
            return;
        }
        if let Some(n) = self
            .world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&self.npc)
        {
            n.vars.insert(key.to_string(), value);
        }
    }

    /// `player.getActiveWeaponInstance()`'s item id, or 0 when bare-handed.
    pub fn equipped_weapon_id(&self) -> i32 {
        self.world
            .objects
            .get_component::<Inventory>(&self.player)
            .map(|inv| inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand))
            .unwrap_or(0)
    }

    /// Quest 415's `checkWeapon`: **bare hands or a fist weapon**
    /// (`weapon == null || FIST || DUALFIST`). Note this is the *inverse*
    /// shape of quests 401/403, which demand one specific weapon id — an Orc
    /// Monk fights unarmed, so "no weapon" is the pass case here, not the
    /// fail case.
    pub fn is_bare_or_fist_handed(&self) -> bool {
        let weapon = self.equipped_weapon_id();
        if weapon == 0 {
            return true;
        }
        matches!(
            self.world.data.item_data.weapon_type(weapon),
            crate::data::item_data::WeaponType::Fist | crate::data::item_data::WeaponType::DualFist
        )
    }

    /// `npc.broadcastPacket(new NpcSay(npc, NPC_GENERAL, npcStringId))`.
    pub fn npc_say(&mut self, npc_string_id: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::npc::say::npc_say(self.world, self.npc, npc_string_id);
    }

    /// `attacker.sendPacket(new NpcSay(npc, NPC_GENERAL, npcStringId))` — the
    /// same line as [`Self::npc_say`] but delivered to the acting player only.
    /// Quest 403's Cat's Eye Bandit taunts its attacker this way while its
    /// death line broadcasts, so the two are not interchangeable.
    pub fn npc_say_to_player(&mut self, npc_string_id: i32) {
        let Some(npc) = self
            .world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
        else {
            return;
        };
        let pkt = server_packets::npc_say(self.npc, npc.npc_id, npc_string_id);
        self.send(pkt);
    }

    /// `Quest.getAlreadyCompletedMsg` (`data/html/alreadycompleted.htm`).
    pub fn already_completed_html(&self) -> String {
        crate::data::htm_cache::read_htm_for(
            self.world,
            self.player,
            format!("{}data/html/alreadycompleted.htm", self.world.data.root),
        )
        .unwrap_or_else(|| {
            "<html><body>This quest has already been completed.</body></html>".to_string()
        })
    }

    /// `Quest.getHtm(player, filename)` — the raw html content of one of the
    /// script's files, for scripts that must substitute a placeholder before
    /// returning it (e.g. quest 234's `%weaponname%`). Returning the result to
    /// the framework sends it inline (`showResult`'s `<html>` branch). Empty
    /// string if the file is missing, matching Java's null collapsing to "".
    pub fn get_htm(&self, filename: &str) -> String {
        load_quest_html(self.world, self.player, &self.script, filename).unwrap_or_default()
    }

    /// `ItemData.getInstance().getTemplate(id).getName()` — an item's display
    /// name, empty if the id is unknown.
    pub fn item_name(&self, item_id: i32) -> String {
        self.world
            .data
            .item_data
            .get(item_id)
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    /// `Quest.startQuestTimer(name, time, npc, player)` — non-repeating
    /// only. Starting a timer with a live same-key predecessor supersedes it
    /// (Java refuses duplicates instead; superseding is the safer default
    /// for the seq scheme and no shipped script relies on the refusal).
    pub fn start_quest_timer(&mut self, name: &str, delay_ms: u64) {
        if self.simulated {
            return;
        }
        let seq = self.world.next_request_seq();
        let quest = self.script.name();
        {
            let timers = self
                .world
                .objects
                .get_component_mut::<QuestTimerSeqs>(&self.player);
            match timers {
                Some(t) => {
                    t.0.insert((quest, name.to_string()), seq);
                }
                None => {
                    let mut map = QuestTimerSeqs::default();
                    map.0.insert((quest, name.to_string()), seq);
                    self.world.objects.add_components(&self.player, map);
                }
            }
        }
        let fire_at = self.world.tick + ms_to_ticks(delay_ms);
        self.world.scheduler.schedule(
            fire_at,
            ScheduledTask::QuestTimer {
                quest,
                name: name.to_string(),
                player: self.player,
                npc: self.npc,
                seq,
            },
        );
    }

    /// `QuestTimer.cancel`: bump the stored seq so the scheduled task
    /// no-ops.
    pub fn cancel_quest_timer(&mut self, name: &str) {
        let seq = self.world.next_request_seq();
        if let Some(t) = self
            .world
            .objects
            .get_component_mut::<QuestTimerSeqs>(&self.player)
        {
            t.0.insert((self.script.name(), name.to_string()), seq);
        }
    }
}
