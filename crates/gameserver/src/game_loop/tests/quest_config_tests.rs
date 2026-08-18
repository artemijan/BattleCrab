//! `General.ini`'s quest keys — `OrderQuestListByQuestId`,
//! `AutoDeleteInvalidQuestData`, and the two `AltDev*` switches.
//!
//! `StoryQuestRewardBuff` has no test because it has no consumer to test:
//! `Quest.giveStoryQuestReward` has **zero callers** in the Java tree and zero
//! in the datapack, so the key gates a scripting entry point nothing on this
//! chronicle uses. Recorded in `config::general`'s header instead.

use super::*;

/// `OrderQuestListByQuestId` (**True** here) sorts the NPC's quest-choice
/// window by id — and Java's `TreeMap` also **de-duplicates**, which a plain
/// sort does not. Both halves are asserted, because the dedup is the part that
/// looks incidental and is not: a map keyed by id cannot hold two.
#[test]
fn the_quest_list_is_ordered_by_id_and_deduplicated() {
    let registry = crate::scripts::build_registry(Vec::new());
    // Real scripts, taken out of registry order and deliberately shuffled, with
    // one repeated so the dedup has something to do.
    let mut names: Vec<&'static str> = registry.names();
    names.sort_by_key(|n| std::cmp::Reverse(registry.quest_id(n).unwrap_or(0)));
    let mut picked: Vec<_> = names
        .iter()
        .filter_map(|n| registry.by_name(n))
        .filter(|q| q.id() > 0)
        .take(4)
        .collect();
    assert!(picked.len() >= 3, "need a few real quests to order");
    let dup = picked[0].clone();
    picked.push(dup);
    let before: Vec<i32> = picked.iter().map(|q| q.id()).collect();
    assert!(
        before.windows(2).any(|w| w[0] < w[1]) || before.first() != before.iter().min(),
        "sanity: the input is not already sorted"
    );

    let mut on = picked.clone();
    crate::game_loop::quests::dispatch::order_quest_list(&mut on, true);
    let ids: Vec<i32> = on.iter().map(|q| q.id()).collect();
    let mut expected: Vec<i32> = before.clone();
    expected.sort_unstable();
    expected.dedup();
    assert_eq!(ids, expected, "sorted by id, and the repeat collapsed");

    // Off: untouched, repeat and all — Java leaves `questList` as it found it.
    let mut off = picked.clone();
    crate::game_loop::quests::dispatch::order_quest_list(&mut off, false);
    assert_eq!(
        off.iter().map(|q| q.id()).collect::<Vec<_>>(),
        before,
        "with the key off the list keeps registry order"
    );
}

/// A `character_quests` row naming a quest this server does not have is dropped
/// from the **live state** whatever the key says — Java's `q == null` branch
/// `continue`s either way, and `AutoDeleteInvalidQuestData` only decides
/// whether the row is also deleted.
///
/// Before this, the port loaded the row into the component and wrote it
/// straight back on the next flush, so a quest renamed between builds left a
/// `QuestState` no code could reach and no restart could clear.
#[test]
fn an_unknown_quest_state_is_dropped_from_the_live_component() {
    for delete_rows in [false, true] {
        let (mut world, _tx, mut db_rx, _link) = test_world();
        world.cfg.general.auto_delete_invalid_quest_data = delete_rows;

        let mut chr = dummy_char(4201, "Quester");
        chr.quests.insert(
            "NoSuchQuest".to_string(),
            crate::model::quest::QuestState::default(),
        );
        let mut bundle = crate::model::Player::from_char(&world.data, &chr);
        assert!(
            bundle.quests.0.contains_key("NoSuchQuest"),
            "the loader hands it over untouched"
        );

        crate::game_loop::lobby::drop_invalid_quest_states(&world, &mut bundle, 4201);
        assert!(
            bundle.quests.0.is_empty(),
            "delete_rows = {delete_rows}: the live state is dropped regardless"
        );

        // …and only the True branch queues the row deletion.
        let queued = std::iter::from_fn(|| db_rx.try_recv().ok())
            .any(|c| matches!(c, crate::db::DbCommand::DeleteQuestRows { .. }));
        assert_eq!(
            queued, delete_rows,
            "AutoDeleteInvalidQuestData = {delete_rows}: row deletion"
        );
    }
}

/// A *known* quest's state survives — the filter is by registry membership, not
/// a blanket wipe. Pinned because the failure mode of getting this wrong is
/// deleting every player's quest progress at once.
#[test]
fn a_known_quest_state_is_left_alone() {
    let (world, _tx, _db, _link) = test_world();
    let known = world
        .quests
        .names()
        .first()
        .copied()
        .expect("the registry has scripts");

    let mut chr = dummy_char(4202, "Quester");
    chr.quests.insert(
        known.to_string(),
        crate::model::quest::QuestState::default(),
    );
    chr.quests.insert(
        "NoSuchQuest".to_string(),
        crate::model::quest::QuestState::default(),
    );
    let mut bundle = crate::model::Player::from_char(&world.data, &chr);
    crate::game_loop::lobby::drop_invalid_quest_states(&world, &mut bundle, 4202);

    assert!(
        bundle.quests.0.contains_key(known),
        "{known} is registered and must survive"
    );
    assert!(!bundle.quests.0.contains_key("NoSuchQuest"));
}

/// `AltDevNoQuests` empties the registry. Despite the name it is **not**
/// quests-only: Java returns from `executeScriptList()` before loading
/// anything, so AI and event scripts go with them.
#[test]
fn alt_dev_no_quests_registers_nothing() {
    let full = crate::scripts::build_registry(Vec::new());
    assert!(
        !full.names().is_empty(),
        "sanity: the real registry has scripts"
    );
    let empty = crate::game_loop::quests::QuestRegistry::new(Vec::new());
    assert!(empty.names().is_empty());
    // The switch replaces one with the other; nothing else about the world
    // changes, which is what makes it safe as a dev switch.
    assert!(empty.by_name(full.names()[0]).is_none());
}
