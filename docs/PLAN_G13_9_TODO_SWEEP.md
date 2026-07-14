# G13.9 — TODO Parity Sweep

Status: **complete** (all three tiers landed). A focused milestone that closes the *buildable-now* TODO
markers left in the Rust source — the small correctness/completeness gaps whose
backing subsystems already exist. This is **not** a "resolve every TODO" pass:
the subsystem-scale deferrals (henna, mail, castle/manor, subclasses, combat
breadth, admin G13.B) stay in their own milestones and are enumerated in §4 so
the split is explicit.

Companion to [PLAN_G13_ADMIN.md](PLAN_G13_ADMIN.md); slots between G13 and G14
in [PROGRESS.md](PROGRESS.md).

## 0. Scope philosophy

A grep for `todo|fixme|deferred|for now|unimplemented` across `crates/` returns
~40 markers. Most are **honest deferrals to a future milestone** — a block
written empty because the subsystem behind it (mail, siege, subclasses) isn't
ported. Sweeping those means building the subsystem, which is the future
milestone, not this one.

What *is* in scope: markers where the data and subsystems already exist and the
TODO is a genuine gap against the Java reference — a value written as 0 that we
can now compute, a silent drop where Java sends a SystemMessage, a persistence
path we skipped. Seven such items, in three tiers. Each is a self-contained,
faithful port with no engine work.

**Explicitly out of scope (correct as-is, do not touch):** Java's own
`// TODO: Find me!` unknown wire bytes (`char_info.rs:124`, `party.rs:35`,
`movement.rs:28`) — we mirror Java exactly; "fixing" them breaks parity.
UserInfo `ELEMENTALS` / `SLOTS` (talisman/brooch) — not present in Interlude
Classic; empty is correct.

---

## 1. Tier 1 — Packet completeness (subsystems already exist) — ✅ landed

### 1.1 UserInfo `ENCHANTLEVEL` — `network/user_info.rs` — ✅ (weapon; armor deferred)
Java (`UserInfo.java:183`) writes two bytes: `getWeaponEnchant()` then
`getArmorMinEnchant()`.
- **Weapon enchant** = R-hand paperdoll item's `enchant_level` — done, reusing
  the existing `Inventory::paperdoll_enchant_level(PaperdollSlot::RHand)` (the
  same call CharInfo already uses for its weapon-enchant byte).
- **Armor min-enchant** — **left at 0, correctly.** Investigation showed Java's
  `getArmorMinEnchant` = `PaperdollCache.getMaxSetEnchant`, which iterates
  `ArmorSetData` and returns **0** when no recognized armor *set* is equipped.
  `ArmorSetData` is unported, so the faithful value is 0 for every player today.
  This moves to §4 (lands with armor sets) — it is **not** a min-across-armor
  approximation as the original draft guessed; that would diverge from Java.

### 1.2 UserInfo `RELATION` — `network/user_info.rs` — ✅
Java `calculateRelation`: `0x08` party member · `0x10` party leader · `0x20`
clan member · `0x40` clan leader · `0x80` in-siege. Added
`game_loop::party::calculate_relation(world, &Player)` — party bits off the
`PartyRef` component + `Party::is_leader`, clan bits off `Player.clan_id` /
`clan_leader`. Siege (`0x80`) unported → stays clear. `user_info()` gained a
`relation: i32` parameter; all callers compute and pass it (the enter-world
burst passes clan-only, since party membership isn't persisted across relog).

### 1.3 CharInfo — no relation field (corrected)
**The original Tier 1.3 was a mistake.** Java `CharInfo` carries *no* relation
int — the nearby-player relation is delivered by the separate `RelationChanged`
packet (already ported). CharInfo instead carries two enchant bytes:
`_enchantLevel` (weapon) — **already** written from `RHand` — and `_armorEnchant`
— the same `getMaxSetEnchant` value, correctly 0 until armor sets (§4). So
CharInfo needed no change.

**Tests:** `relation_reflects_party_and_clan` (game_loop tests) covers the
clan-member/leader and party-member/leader mask combinations; the
`user_info_packet` golden test updated to pass `relation: 0` (its fixture is
clanless with an empty inventory, so the golden bytes are unchanged).

---

## 2. Tier 2 — Small behavioral gates — ✅ landed

### 2.1 Skill-acquire level/SP SystemMessage — `game_loop/skills/mod.rs` — ✅
The combined silent guard split into Java `checkPlayerSkill`'s two branches:
level first → `YOU_DO_NOT_MEET_THE_SKILL_LEVEL_REQUIREMENTS` (id 2208), then SP
(`level_up_sp > 0 && level_up_sp > sp`) → `YOU_DO_NOT_HAVE_ENOUGH_SP_TO_LEARN_THIS_SKILL`
(id 278). Both SM ids added to `sm_ids`. Test:
`skill_acquire_gates_send_system_messages`.

### 2.2 RestorationItem enchant roll — `model/skill.rs` — ✅
`give_item_random` now rolls `Rnd.get(min_enchant, max_enchant)` (inclusive) when
`max_enchant > 0` and stamps it on the created **non-stackable** item via a new
`Inventory::set_item_enchant`; `give_item` applies the fixed `item_enchant_level`.
The single-enchanted grant uses Java's `YOU_HAVE_OBTAINED_A_S1_S2` ("obtained a
+S1 S2", id 369) message. Test:
`item_skill_give_item_random_rolls_enchant_on_created_item`.

### 2.3 Config plumbing for stat caps / run-speed boost — `model/mod.rs` — ✅ (scope corrected)
**The original estimate ("thread the CharacterConfig already passed into
`user_info`") was wrong** — `recalculate_stats` / `recompute_npc_stats_from_buffs`
never receive `CharacterConfig`, and it had no fields for these anyway. Done as a
clean injection instead of a pipeline-wide signature ripple: added the 8 keys
(`RunSpeedBoost` + the `Max*` ceilings) to `CharacterConfig` (parsed from
Character.ini), and a `GameData::combat_caps: CombatCaps` bundle the stat engine
already reaches through its `&GameData`. `main.rs` folds the parsed config into
`combat_caps` at boot; the module-level `MAX_*`/`RUN_SPD_BOOST` consts are gone.
Defaults equal this dist's values, so behavior is unchanged (all 325 stat/other
lib tests green) — the win is that a deployment override now actually takes
effect. The one NPC caller passes `&world.data.combat_caps`.

---

## 3. Tier 3 — Persistence gap — ✅ landed (item corrected)

### 3.1 Skill reuse cooldowns persist across relog — `model/components.rs:228` — ✅
**The plan mislabeled this as "quest timers."** The actual `components.rs:228`
TODO is on `Reuses` (skill cooldowns): *"persist across relog like Java's
`character_skills_save`."* Quest timers are a non-issue — they're explicitly
"not persisted, like Java" (`components.rs:395`), so there was never a TODO
there. And this dist runs `StoreSkillCooltime = True`, so wiping cooldowns on
relog was a real behavioral gap.

Implemented the `character_skills_save` **reuse half** (Java `storeEffect`/
`restoreEffects`, `restore_type = 1`; buff restore / `restore_type = 0` stays
deferred with `db.rs:78`):
- **Config**: `StoreSkillCooltime` added to `CharacterConfig` (default true).
- **Store** (`net.rs` → `db.rs`): `PlayerSaveData`/`CharData` gained
  `skill_reuses: Vec<SkillReuseRow>`. Because `until_tick` is server-uptime
  relative, the flush persists an **absolute wall-clock `systime`** (Java
  `TimeStamp.getStamp()`) so cooldowns decay by real elapsed time across a
  relog/restart. `store_player_tx` always delete-then-inserts (an empty set —
  config off, or no cooldowns — clears the rows). The reuse-map key goes in the
  `skill_id` column (Java-compatible for ungrouped skills; it's the value the
  map is re-keyed by on restore).
- **Load** (`db.rs`): `load_skill_reuses` reads `restore_type = 1` rows, drops
  already-expired ones.
- **Restore** (`model/mod.rs`): `PlayerData::restore_reuses` re-arms the live
  `Reuses` map off the current game tick (`systime − now → until_tick`), called
  on the real select path — so the many `from_char` callers stay unchanged. The
  enter-world `SkillCoolTime` now reflects the restored cooldowns.
- The in-repo dev DB already has `character_skills_save`, so no schema step.
- **Tests**: `skill_reuse_cooldown_survives_relog` (game-loop: save→relog→restore
  round-trip, incl. config-off clears) and `skill_reuse_cooldowns_persist`
  (char_persistence: real DB round-trip, future systime survives / past filtered
  / empty flush clears).

---

## 4. Deferred — stays in its own milestone (documented, not swept)

| TODO cluster | Location | Belongs to |
|---|---|---|
| Admin command bodies (res_monster NPC, NPC-target heal/teleport/recall, permanent/respawn spawns, immediate-passive path, class-transfer cleanup, kill-by-name/radius) | `game_loop/admin.rs` ×9 | **G13.B** (current admin milestone) |
| Henna / Mail empty lists | `enter_world.rs`, `lobby.rs` | G14 (needs subsystems) |
| Manor / castle list / siege | `server_packets/manor.rs` ×3, `user_info.rs` siege bit | G-later (CastleManager) |
| Subclasses + buff-restore-on-login | `db.rs:78` | G-later |
| Combat breadth (split hits, polearm sweeps, soulshots, shield blocks, force-attack PvP) | `game_loop/combat.rs` | G14 |
| Regen sitting/running multipliers | `game_loop/regen.rs:46` | with a posture/SitStand feature |
| Armor min-enchant byte (UserInfo + CharInfo) — needs `ArmorSetData` (`getMaxSetEnchant`) | `user_info.rs`, `char_info.rs` | with armor sets (G14) |
| Data breadth (female collision, Orc Fighter 2nd class, zone `type=` parse, teleport-home flag) | `player_template.rs`, `category_data.rs`, `zone_data.rs` | G14 long tail |
| Enter-world store/warehouse limits (Java defaults hardcoded) | `enter_world.rs:407-413` | cosmetic; fold into store subsystem |

---

## 5. Order of work & verification

1. **1.2 + 1.3 relation** (shared helper) → verify UserInfo/CharInfo mask in a
   party and in a clan against Java field-by-field.
2. **1.1 enchant** → verify weapon-enchant byte with an enchanted weapon
   equipped; armor byte with an armor set.
3. **2.1 skill SM** → drive an under-level / under-SP acquire, assert the SM.
4. **2.2 restoration enchant**, **2.3 config** → unit-level.
5. **3.1 quest-timer persistence** → relog with an active quest timer, assert it
   resumes.

Each tier is independently shippable. `e2e_create` already asserts the
enter-world burst; extend it to cover the relation mask and enchant bytes rather
than adding a parallel harness. There is no linter — `cargo build` +
targeted test filters (per the memory note: don't run the full gameserver suite
from the shell) are the gate.
