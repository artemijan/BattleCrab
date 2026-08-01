# G33 — the `Custom/*.ini` enable-flag audit

[ROADMAP.md](ROADMAP.md)'s scope gate defers the Mobius `config/Custom/*`
features as **out of scope** — "except any the operator explicitly enables —
G33 includes a one-time audit of `Custom/*.ini` enable flags to finalize this."

G33 was marked complete without that audit ever running. This is it.

## Method

Three questions per file, because a shipped ini proves nothing on its own —
`Custom/PcCafe.ini` is the standing counter-example: it ships, it looks
authoritative, and **`Config.java` never opens it** (the live PC-cafe keys are
in `PremiumSystem.ini`).

1. **Is the flag on in `dist/game/config/Custom/`?** The dist is the
   specification; an operator-enabled flag is operator intent.
2. **Does `Config.java` parse the file?** `grep "Custom/<name>.ini"`.
3. **Does anything in Java *consume* the parsed constant?** `grep` the constant
   across `java/` + `dist/game/data/scripts/`, excluding `Config.java` itself.

A feature has to answer yes three times before it counts as a real gap. Then:
does the Rust port read the key at all (`grep` the key name across `crates/`)?

## Result

**17 features are enabled on this dist, live in Java, and absent from the port.**
One file (`PcCafe.ini`) is confirmed dead in Java itself.

### Enabled + live in Java + missing in the port

| Feature | Master flag | Java consumers | Notes |
|---|---|---|---|
| **Champion monsters** | `ChampionEnable = True` | `Attackable`, `Creature`, `AttackableAI`, `NpcTemplate`, 4 stat finalizers | ✅ **ported in this branch** |
| Banking | `BankingEnabled = True` | `MasterHandler` (voiced command) | adena ↔ goldbar |
| Custom mail manager | `CustomMailManagerEnabled = True` | `GameServer` | DB-polled mail delivery |
| Auto-play | `EnableAutoPlay = True` | `AutoPlayTaskManager`, `Player`, `MasterHandler` | Classic auto-hunt; largest of the set |
| Auto-potions | `AutoPotionsEnabled = True` | `AutoPotion`, `MasterHandler` | |
| Boss announcements | `RaidBossSpawnAnnouncements`, `GrandBossSpawnAnnouncements` | `Creature` | spawn-only on this dist; defeat flags are off |
| Chat moderation | `ChatAdmin = True` | `MasterHandler` | `//chatban`-adjacent voiced command |
| Nobless master | `Enabled = True` | `NoblessMaster` script | NPC that grants nobless |
| Online info | `EnableOnlineCommand = True` | `MasterHandler`, `Online` | `.online` player count |
| PvP reward item | `RewardPvpItem = True` | `Npc`, `Player` | 300 000 adena per PvP kill here |
| PvP title colour | `EnablePvPColorSystem = True` | `Player` | title/name colour ladder by PvP count |
| Random spawns | `EnableRandomMonsterSpawns = True` | `Spawn`, `AbstractScript` | ±100 unit spawn jitter |
| Sell buffs | `SellBuffEnable = True` | `GameServer`, `SellBuffsManager`, `SellBuff` | player buff shops |
| L2Walker protection | `L2WalkerProtection = True` | `Say2` | bot-client detection |
| Private store range | `ShopMinRangeFromPlayer/Npc` | `Player` | 50 / 100 unit shop spacing |
| Dualbox check | `DualboxCheckMaxPlayersPerIP = 2` | `CharacterSelect` | 2 clients per IP |
| Allowed player races | `AllowHuman`…`AllowDwarf` | `CharacterCreate` | **all five True → currently a no-op**, but the gate is missing |

### Correctly out of scope — the flag is off on this dist

`FactionSystem` (`EnableFactionSystem = False`), `FakePlayers`, `FindPvP`,
`MerchantZeroSellPrice`, `MultilingualSupport`, `NpcStatMultipliers`,
`OfflinePlay` (`EnableOfflinePlayCommand = False`), `PasswordChange`,
`SayuneForAll`, `ScreenWelcomeMessage`, `StartingLocation`
(`CustomStartingLocation = False`), `DelevelManager`,
`CustomDepositableItems`, `PvpAnnounce` (`AnnouncePkPvP = False`).

`ClassBalance.ini` ships with **every multiplier list empty**, so it is inert
even though the file is parsed.

### Already ported

`CommunityBoard.ini` (G30), `OfflineTrade.ini` (G33), `PremiumSystem.ini`
(G16), `SchemeBuffer.ini` (G30), `ServerTime.ini` (`DisplayServerTime`, in
`user_commands`).

### Dead in Java

`Custom/PcCafe.ini` — **`Config.java` does not parse it**. Its keys duplicate
live ones in `PremiumSystem.ini`, which is what the PC-cafe code actually
reads. Nothing to port; do not be misled by the file's existence.

## Slice 1 — champion monsters (this branch)

Everything Java's `_champion` flag reaches, ported:

- **`config/champion.rs`** — the whole `CUSTOM_CHAMPION_MONSTERS_CONFIG_FILE`
  block, with Java's `Config` defaults for an absent file.
- **The lottery** (`model::npc::roll_champion`, Java `Attackable.onRespawn`):
  the full guard chain (monster subtree, not a quest monster, not undying, not
  a raid, not a raid minion, frequency > 0, inclusive level window, instance
  gate) plus `Rnd.get(100) < ChampionFrequency`. Rolled **per instance**, so a
  respawn re-rolls, and on the global `rnd` stream so it cannot shift the
  forced-roll sequences combat tests depend on.
- **Two new template flags** the gate needs: `<status undying>` (681 NPCs on
  this dist, previously unparsed) and `isQuestMonster()`, which Java computes
  as `title.contains("Quest")` rather than reading from XML.
- **Stats** — `ChampionAtk` on P.Atk/M.Atk and `ChampionSpdAtk` on both attack
  speeds, threaded as `ChampionStatMods` so the stat layer stays a pure
  function and a **buff recompute keeps the multipliers** (getting this wrong
  would strip a champion's P.Atk the first time anything buffed it).
  `ChampionHpRegen` in `regen.rs`; there is deliberately no MP equivalent,
  because `RegenMPFinalizer` has no champion arm in Java either.
- **Damage** — `Creature.reduceCurrentHp`'s divisor. Max HP is untouched:
  Java models a champion's bulk as `damage / ChampionHp`, so the health bar
  still reads 100 % and hate is still computed from the *undivided* damage.
- **Rewards** — `ChampionRewardsExpSp` on exp and sp in both the solo and
  party branches; the drop chance/amount multipliers with Java's **two-arm
  split** preserved (the adena multipliers fire only inside the
  `RATE_DROP_CHANCE_BY_ID` branch, the generic ones only in the flat `else`);
  and the `ChampionRewardItems` tail.
- **`useVitalityRate()`** — now real. It gates three things at once: the bonus
  multiplier argument to `addExpAndSp`, the vitality charge, and the PA-point
  award. Previously hard-coded `true` with a comment saying champions were not
  ported.
- **Presentation** — the `Champion` title (Java has *two* arms: the decorated
  branch prefixes, the plain branch replaces) and the `Team.RED` aura as a new
  `TEAM` component in `NpcInfo`, written between `MOVE_MODE` and `ENCHANT`.
- **AI** — `ChampionPassive` stops the aggro scan seeding hate.

### Java quirks kept deliberately

- **`ChampionRewardLowerLvlItemChance` / `…HigherLvlItemChance` are inverted.**
  The ini documents them as "% Chance to obtain", but both Java arms `return`
  *before* adding the reward, so each is really a **suppression** chance. With
  this dist's `lower = 0` / `higher = 100`, a champion below your level always
  pays the reward item and one above your level never does — the opposite of
  what the comment promises. Ported behaviour-first, documented at the site.
- **The reward-item guard is `containsAll`, not per-item.** A champion that
  rolled *some* of a multi-item reward list gets the whole list appended,
  duplicating what it already had.
- **`ChampionEnable` is re-checked at every consumer**, not just at the roll,
  so flipping the master flag off makes already-spawned champions behave like
  ordinary mobs immediately.

### Verification

21 tests (4 config, 17 behavioural). Sabotage-verified one at a time: the
damage divisor, the passive-AI gate, the reward-item tail, the team aura, and
the stat multipliers each fail their test when broken.

**One test was found vacuous by that pass** and fixed: the passive-AI test's
second half re-ticked a mob whose intention the *first* half had already moved
to `Attack`, so the second tick ran the attack loop instead of the aggro scan
and the assert held no matter what the champion gate did. Resetting the
intention to `Active` makes it real — it now fails under sabotage.

## Slice 2 — the six cheap features

All six of tier 1, in one `config/custom_misc.rs` (none is more than a handful
of keys):

- **`.online`** (`EnableOnlineCommand`) — the population line, Java's
  singular/plural split kept. Counts in-game sessions **plus standing offline
  shops**, which is what `World.getPlayers()` returns in Java.
- **Banking** (`BankingEnabled`) — `.bank` / `.deposit` / `.withdraw`, adena ↔
  goldbar at 1 000 000 000 : 1 here. Java's `updateDatabase()` has no
  equivalent: the port is memory-first and the inventory rides the autosave.
- **L2Walker protection** — a **whisper** opening with one of the eight bot
  verbs kicks the sender (`DefaultPunish = KICK`). Gated on `ChatType.WHISPER`
  like Java, so the same text said aloud is ordinary chat.
- **Boss announcements** — the spawn line, in chat **and** on screen, for a
  raid or grand boss. Both defeat flags ship `false`, so only the spawn arm
  exists; the name comes from `NpcData` rather than the instance's title, so a
  champion prefix never leaks into it.
- **Private store range** — the port had no `canOpenPrivateStore` gate at all.
  Added, with the spacing rule as its first half: `ShopMinRangeFromNpc` from any
  NPC, `ShopMinRangeFromPlayer` from another **seated** player only (Java's
  `getMinShopDistance` returns 0 unless sitting, so the rule spaces shops apart
  rather than blocking on a passer-by). `_isSellingBuffs` and the `NO_STORE`
  zone are the two legs still absent — the former is this audit's sell-buffs
  slice, the latter a zone kind the port does not load.
- **Allowed player races** — the per-race `switch` in `CharacterCreate`. All
  five are `True` here, so it is inert; it exists so an operator turning one off
  is obeyed rather than ignored.

**One design note.** Java announces a boss from `Npc.onSpawn`, and excludes
minions with `!isMinion() && !isRaidMinion()`. The port cannot test that at the
same point: `MinionOf` is attached *after* the entity exists, so a check inside
the spawn would be dead code. Suppression therefore lives at the call site —
minions spawn through a new `spawn_minion_npc_at`, which does not announce. The
champion code two lines away has the same shape for the same reason.

8 tests, 5 mechanisms sabotage-verified.

## Remaining slices

Ten features, rough order by cost:

1. **Moderate:** PvP reward item, PvP title colour, dualbox check (the
   `CharacterSelect` per-IP cap — the *event* cap and the whole
   `DualboxCheck.ini` parse already landed with the TvT anti-feed slice),
   random spawns, chat moderation, nobless master.
2. **Larger:** sell buffs (a new store type), custom mail manager (a DB poll
   loop), auto-play + auto-potions (Classic auto-hunt, its own packet family).
