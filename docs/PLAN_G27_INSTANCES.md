# PLAN — G27 Instances

Instances are logical world partitions: every object carries an `instanceId`,
and two objects interact (see each other, receive each other's broadcasts) only
when their instance ids match. Instance 0 is the shared overworld. Java
`instancemanager/InstanceManager` + `model/instancezone/{Instance,
InstanceTemplate}`; the gate is `AdminInstance`/`AdminInstanceZone`.

Interlude instance content is thin: the four Olympiad arenas
(`data/instances/Olympiad/*`) and Frintezza's Last Imperial Tomb
(`data/instances/Bosses/LastImperialTomb.xml`). The Olympiad arenas are the
motivating first case — G25 matches currently share one grassy-arena coordinate,
so concurrent matches overlap; instances fix that.

## Slices

1. **The instance partition (this slice).** `InstanceId(i32)` component (absent =
   overworld 0); `instance_of`. Player visibility (`on_enter_world`,
   `update_region`) and `broadcast_to_others` gated on matching instance ids.
   `World.instances`: an id allocator + active-instance registry, with
   `create_instance` / `destroy_instance`. Olympiad matches each take a fresh
   instance so concurrent bouts no longer overlap (closes the G25 stadium gap).
2. **NPC / door / ground-item visibility + the remaining broadcasts** scoped by
   instance, so instanced content (spawns) is private to its instance.
3. **`InstanceTemplate` loading** (`data/instances/*.xml`): spawns, doors, exit
   location, reenter/removeBuff rules; `create_instance(template_id)` populates
   the instance from the template.
4. **Enter/exit + lifetime**: teleport a party into an instance, the exit
   location, the empty-destroy timer, reenter-time tracking.
5. **`AdminInstance` / `AdminInstanceZone`** GM commands.
6. **Content**: wire the Olympiad arenas + Frintezza's tomb onto the framework.
