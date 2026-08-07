# Custom dist deviations

Places where `dist/game` — the datapack under `data/`, and the `.ini` files
under `config/` — **intentionally differs** from retail / upstream
`L2J_Mobius_Classic_Interlude`, by operator decision rather than by porting
accident.

The dist data is otherwise treated as the specification: when the server
behaves differently from what the data implies, the bug is in the server, not
in the data. That rule only works if the handful of deliberate exceptions are
written down — otherwise a future "the data must be wrong" or a re-sync from
the Java reference repo silently reverts them.

Each entry names the files, what retail does, and the test that fails if the
change is dropped.

## Cruma Tower — the entrance serves the 3rd basement floor only

- **Files:** `data/teleporters/others/CrumaTower.xml` (npc 30483),
  `data/html/teleporter/30483.htm`
- **Retail:** Carsus (30483), at the tower entrance, offers **two**
  destinations — the 2nd basement floor `17664,108288,-9056` (index 0) and the
  3rd `17726,114838,-11696` (index 1) — with a button apiece on his page.
  Upstream Mobius Classic Interlude / Classic 2.9 / Classic 3.0 /
  GrandCrusade all ship the same two-entry list.
- **Here:** the 2nd-floor destination and its button are removed. The entrance
  drops players on the **3rd floor only**; the 2nd floor is reached onward
  from there through Ivory Tower Wizard Rombel (30487), who stands at
  `17811,114750,-11680` on the 3rd floor and whose sole destination is the 2nd
  floor. Nothing else in the chain changes: Belkadhi (30485) still runs 2nd →
  3rd/entrance, Janssen (30484) still runs 3rd → entrance, and Ian (30486) is
  still the only holder of the 1st floor, from the far end of the 2nd.
- **Index trap:** the shipped html buttons address destinations by *list
  index*, so dropping the 2nd-floor line moved the 3rd floor from index 1 to
  index 0 — `30483.htm`'s surviving button was retargeted to `OTHER 0` in the
  same change. Re-adding the 2nd-floor line without also fixing the html would
  not error; it would quietly send that button to the 2nd floor.
- **Guarded by:** `game_loop::tests::cruma_tower_tests` — the entrance list
  contents, the page's single button walked end to end onto the 3rd floor,
  Rombel's list and spawn, and Ian's original route.

## Quest 214 — the Ivory Tower gargoyle is "Reinforced", not "Enhanced"

- **Files:** `data/scripts/quests/Q00214_TrialOfTheScholar/30612-04.html`
  (plus a client-side `QuestName` edit made by the operator, outside this repo)
- **Retail:** mob 20567 and item 2719 are named **"Reinforced Gargoyle"** and
  **"Reinforced Gargoyle's Nail"** in `stats/npcs/20500-20599.xml` and
  `stats/items/02700-02799.xml`, and the client's `ItemName` table agrees. But
  Casian's page calls them **"Enhanced Gargoyle Nails"** / **"Enhanced
  Gargoyles"**, and the client's `QuestName` journal entry for quest 214 step
  28 calls them **"Enchanted Gargoyles"**. Three different names for one mob
  across five surfaces, all shipped that way upstream.
- **Here:** everything says **"Reinforced"**. `30612-04.html` was rewritten to
  match the data, and the operator renamed the client `QuestName` entry to
  match too. The npc and item xml were already correct and are untouched.
- **Why this one is a deviation and not a bug fix:** nothing was *broken* —
  the quest always tracked the right ids, and the port reproduced Java exactly.
  What failed was the player's ability to act on the instructions: the journal
  named a monster that appears nowhere in the world, so the errand read as
  impossible. Only three of the five surfaces could be fixed server-side, which
  is why this needed a client change to finish.
- **The trap if it is re-synced:** a refresh from the Java reference repo will
  bring "Enhanced Gargoyle Nails" back into `30612-04.html`, and the page will
  then disagree with both the item table and the (patched) journal. The
  `interlude_classic` reference dist still ships the retail wording.
- **Guarded by:** `game_loop::tests::quests_tests::quest_q00214_trial_of_the_scholar`
  — the reagent page asserts the "Reinforced" wording.

## Advanced Headquarters takes half damage, not one and a half times

- **Files:** `game_loop/combat/damage.rs` (the halving),
  `model/components.rs` (`AdvancedHeadquarter`), `game_loop/siege.rs`
- **Retail:** skill 326 "Build Advanced Headquarters" plants the same flag NPC
  (35062) as the basic skill 247, and `SiegeFlagStatus.reduceHp` reads:

  ```java
  if (isAdvancedHeadquarter()) super.reduceHp(value / 2, …);
  super.reduceHp(value, …);
  ```

  There is no `else` and no `return`, so an advanced camp takes `value/2 +
  value` — **1.5× damage**. The noble-only skill is therefore strictly worse
  than the basic one it upgrades.
- **Here:** an advanced camp takes **half** damage.
- **Why this one is a deviation and not a bug fix:** it is a bug fix, and that
  is the point — the repo's rule is to port behaviour rather than intent, so
  choosing intent has to be written down. Everything about the skill says
  halving: its name, its `autoGet="true"` place in `nobleSkillTree.xml`, and
  the obvious purpose of that `if`. Reproducing the arithmetic faithfully would
  hand nobles a downgrade, which no player would read as correct.
- **The trap if it is re-synced:** nothing in the Java source marks this as a
  bug, so a future parity pass comparing `reduceHp` line by line will see a
  mismatch and "correct" it. That would silently make advanced camps three
  times easier to destroy than they are now.
- **Guarded by:** `game_loop::tests::combat_tests::
  an_advanced_headquarters_takes_half_damage` — asserts 950 HP after a 100
  hit, and names the 1.5× alternative so the intent survives the assertion.

## TvT: a servitor is thawed when the event ends

- **Java** (`custom/events/TeamVsTeam/TvT.java`, the `"EndFight"` teardown).
  The "Disable players" block freezes each participant and their servitors:

  ```java
  participant.setInvul(true);  participant.setImmobilized(true);  participant.disableAllSkills();
  for (Summon summon : participant.getServitors().values()) {
      summon.setInvul(true);  summon.setImmobilized(true);  summon.disableAllSkills();
  }
  ```

  The later "Enable players" block undoes it — for the **player**:

  ```java
  participant.setInvul(false); participant.setImmobilized(false); participant.enableAllSkills();
  for (Summon summon : participant.getServitors().values()) {
      summon.setInvul(true);   summon.setImmobilized(true);   summon.disableAllSkills();  // <- unchanged
  }
  ```

  The inner loop is a verbatim copy of the freeze block, and nothing else in
  the script touches those flags again.
- **Here:** the thaw clears invulnerability, immobilisation and the skill lock
  on the servitor as well as the owner.
- **Why this one is a deviation and not a bug fix:** it is a bug fix, and the
  repo's rule is to port behaviour rather than intent, so choosing intent gets
  written down. A servitor that survives a TvT event in Java is left
  invulnerable and unable to move or cast **for the rest of the session**, with
  no code path that restores it — an outcome the surrounding "Enable players"
  comment plainly does not intend.
- **The trap if it is re-synced:** a line-by-line comparison of the teardown
  will read the port's `false` as a mismatch and "correct" it back, which
  reintroduces permanently broken pets for anyone who brings a summon to the
  event.
- **Guarded by:** `game_loop::tests::tvt_tests::
  end_fight_freezes_players_and_servitors_and_teleport_out_thaws_them` — the
  final assertion names Java's behaviour so the intent survives the assertion.

## `StrictDelevelSkillRemoval` — a config key with no upstream equivalent

- **Files:** `dist/game/config/Character.ini`
- **Retail:** `Player.checkPlayerSkills` applies a **9-level grace**: a known
  skill is only downgraded once the character drops below `learn level − 9`,
  and only removed once even level 1 is out of range that way. There is no key
  to change this — the behaviour is hard-coded, and the shipped
  `DecreaseSkillOnDelevel` comment describes it ("If player level is lower than
  skill learn level - 9…").
- **Here:** `StrictDelevelSkillRemoval` (default **True**) drops the grace, so
  a skill is downgraded or removed the moment the character falls below its
  learn level — the level-exact rule already used for Expertise. Setting it to
  `False` restores the upstream behaviour exactly. It is only consulted when
  `DecreaseSkillOnDelevel` is on.
- **Why it is listed here:** the key is read by
  `config::character::CharacterConfig` and consumed by
  `game_loop::death::progression::maybe_skill_remove_on_delevel`, but it is not
  a port of anything — a reader comparing `Character.ini` against upstream will
  find no counterpart, and the *default* is the non-retail branch. Until this
  entry existed the divergence was recorded only in a Rust doc-comment, and the
  ini did not ship the key at all, so every boot logged
  `missing property for key: StrictDelevelSkillRemoval`.
- **Guarded by:** `game_loop::tests::skills_tests::
  delevel_downgrades_then_removes_skills` — it runs the same delevel under both
  settings, and the non-strict case asserts the 9-level grace keeps a
  `getLevel`-7 skill at level 1 where strict strips it.
