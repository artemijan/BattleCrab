//! Entry points: the `QuestLink` bypass router, the `notify_*` event hooks,
//! the creature-see sweep, quest timers and the list/abort packets.

use super::*;

/// The `QuestLink` bypass handler: `Quest` (chooser), `Quest <Name>`
/// (talk), `Quest <Name> <event>` (html-button event). `command` is the
/// full bypass command starting with `Quest`.
pub(crate) fn quest_link(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    command: &str,
) {
    let rest = command
        .strip_prefix("Quest")
        .map(|r| r.trim())
        .unwrap_or("");
    if rest.is_empty() {
        show_quest_window_all(world, client_id, player, npc_oid);
    } else if let Some((name, event)) = rest.split_once(' ') {
        process_quest_event(world, client_id, player, npc_oid, name, event.trim());
    } else {
        show_quest_window(world, client_id, player, npc_oid, rest);
    }
}

/// `QuestLink.showQuestWindow(player, npc)`: gather the NPC's talk quests →
/// chooser when several, straight to the single one, `noquest.htm` when
/// none. Quests whose simulated `onTalk` would only show the no-quest
/// message are dropped first, exactly as Java does — see
/// [`talk_shows_no_quest`].
fn show_quest_window_all(world: &mut World, client_id: u32, player: i32, npc_oid: i32) {
    let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
    let registry = world.quests.clone();
    // Opted-in utility scripts (`bare_talk`, e.g. TeleportWithCharm) run
    // their `on_talk` from the bare quest-window route; a returned html
    // ends the interaction (see the trait method's deviation note).
    for script in registry.talk_quests(npc_id) {
        if script.id() > 0 || !script.bare_talk() {
            continue;
        }
        let html = {
            let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
            script.on_talk(&mut ctx)
        };
        if let Some(html) = html {
            show_result(world, client_id, npc_oid, &script, Some(html));
            return;
        }
    }
    let candidates: Vec<_> = registry
        .talk_quests(npc_id)
        .into_iter()
        .filter(|q| q.id() > 0 && q.id() < 20000 && q.id() != 255)
        .collect();
    let mut quests = Vec::with_capacity(candidates.len());
    for q in candidates {
        if !talk_shows_no_quest(world, client_id, player, npc_oid, &q) {
            quests.push(q);
        }
    }
    match quests.len() {
        0 => send_no_quest_html(world, client_id, npc_oid),
        1 => show_quest_window(world, client_id, player, npc_oid, quests[0].name()),
        _ => show_quest_choose_window(world, client_id, player, npc_oid, &quests),
    }
}

/// Java's `Quest.getNoQuestMsg(player).equals(quest.onTalk(npc, player,
/// true))` probe: run the script's talk handler on a [simulated] context and
/// report whether all it would produce is `noquest.htm`. Both quest-window
/// routes drop such quests — a quest that has nothing to say at this NPC is
/// not listed at all.
///
/// This is not cosmetic. The chooser labels its buttons with the client
/// strings `<questId>01/02/03`, and the one-time class-change quests only
/// ship `01` ("Path of the Human Wizard") and `02` ("… (In Progress)") —
/// there is no `40403` for the completed state. Listing a finished Q404 at
/// Parina therefore rendered a *blank* grey button that answered
/// `noquest.htm` when clicked. Java never reaches that button because this
/// filter removes the quest first.
///
/// A script returning `None` is kept, matching Java: `equals(null)` is false.
///
/// [simulated]: QuestCtx::new_simulated
fn talk_shows_no_quest(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    script: &Arc<dyn QuestScript>,
) -> bool {
    let res = {
        let mut ctx = QuestCtx::new_simulated(world, client_id, player, npc_oid, script.clone());
        script.on_talk(&mut ctx)
    };
    match res {
        Some(html) => html == no_quest_html(world),
        None => false,
    }
}

/// `QuestLink.showQuestChooseWindow`: one `<button>` per quest, colored and
/// labeled by state (`<fstring>{questId}01/02/03</fstring>` — client-side
/// strings). A single *available* quest short-circuits straight to it.
fn show_quest_choose_window(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    quests: &[Arc<dyn QuestScript>],
) {
    let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
    let registry = world.quests.clone();
    let mut started = String::new();
    let mut can_start = String::new();
    let mut cant_start = String::new();
    let mut completed = String::new();

    let mut start_count = 0;
    let mut start_quest: Option<&'static str> = None;
    for q in quests {
        let button = |sb: &mut String, color: &str, suffix: &str| {
            sb.push_str(&format!(
                "<font color=\"{color}\"><button icon=\"quest\" align=\"left\" \
                 action=\"bypass npc_{npc_oid}_Quest {}\"><fstring>{}{suffix}</fstring></button></font>",
                q.name(),
                q.id(),
            ));
        };
        let qstate = world
            .objects
            .get_component::<Quests>(&player)
            .and_then(|qs| qs.0.get(q.name()))
            .map(|qs| (qs.state, qs.is_started()));
        match qstate {
            None | Some((state::CREATED, _)) => {
                if !registry.is_start_npc(q.name(), npc_id) {
                    continue;
                }
                let eligible = {
                    let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, q.clone());
                    q.start_condition_html(&mut ctx).is_none()
                };
                if eligible {
                    start_count += 1;
                    start_quest = Some(q.name());
                    button(&mut can_start, "bbaa88", "01");
                } else {
                    button(&mut cant_start, "a62f31", "01");
                }
            }
            // Java's `else if (getNoQuestMsg(player).equals(quest.onTalk(npc,
            // player, true))) continue;` sits ahead of both remaining arms: a
            // quest with nothing to say at this NPC gets no button at all.
            _ if talk_shows_no_quest(world, client_id, player, npc_oid, q) => continue,
            Some((_, true)) => {
                start_count += 1;
                start_quest = Some(q.name());
                button(&mut started, "ffdd66", "02");
            }
            Some((state::COMPLETED, _)) => button(&mut completed, "787878", "03"),
            _ => {}
        }
    }

    if start_count == 1 {
        show_quest_window(
            world,
            client_id,
            player,
            npc_oid,
            start_quest.expect("count == 1"),
        );
        return;
    }

    let content = if started.is_empty()
        && can_start.is_empty()
        && cant_start.is_empty()
        && completed.is_empty()
    {
        no_quest_html(world)
    } else {
        format!("<html><body>{started}{can_start}{cant_start}{completed}</body></html>")
    };
    let content = content.replace("%objectId%", &npc_oid.to_string());
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_oid, &content),
    );
    send_action_failed(world, client_id);
}

/// `QuestLink.showQuestWindow(player, npc, questId)` → `Quest.notifyTalk`:
/// the start-condition gate (only when this NPC starts the quest), else
/// `on_talk`. (The weight-penalty / 40-quest guards are unported — no
/// weight model.)
fn show_quest_window(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    quest_name: &str,
) {
    let registry = world.quests.clone();
    let Some(script) = registry.by_name(quest_name) else {
        send_no_quest_html(world, client_id, npc_oid);
        send_action_failed(world, client_id);
        return;
    };
    world.objects.add_components(&player, LastFolkNpc(npc_oid));
    let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
    let res = {
        let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
        let gate = if registry.is_start_npc(quest_name, npc_id) && ctx.is_created() {
            script.start_condition_html(&mut ctx)
        } else {
            None
        };
        match gate {
            Some(html) => Some(html),
            None => script.on_talk(&mut ctx),
        }
    };
    show_result(world, client_id, npc_oid, &script, res);
}

/// `Player.processQuestEvent` → `Quest.notifyEvent` → `onEvent`.
fn process_quest_event(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    name: &str,
    event: &str,
) {
    let registry = world.quests.clone();
    let Some(script) = registry.by_name(name) else {
        warn!("Quest event for unknown quest [{name}].");
        return;
    };
    let res = {
        let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
        script.on_event(&mut ctx, event)
    };
    show_result(world, client_id, npc_oid, &script, res);
}

/// `NpcAction`'s first-talk branch: if a script owns this NPC's chat
/// window, run its `onFirstTalk` and report `true` so the caller skips
/// `Npc.showChatWindow` entirely.
pub(crate) fn notify_first_talk(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    npc_id: i32,
) -> bool {
    let registry = world.quests.clone();
    let Some(script) = registry.first_talk_quest(npc_id) else {
        return false;
    };
    let res = {
        let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
        script.on_first_talk(&mut ctx)
    };
    show_result(world, client_id, npc_oid, &script, res);
    true
}

/// `onAggroRangeEnter`: the aggro scan just seeded first hate on a player
/// inside a registered monster's range.
pub(crate) fn notify_aggro_range_enter(
    world: &mut World,
    npc_oid: i32,
    npc_id: i32,
    player_oid: i32,
) {
    let registry = world.quests.clone();
    let scripts = registry.aggro_enter_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, player_oid) else {
        return;
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, player_oid, npc_oid, script.clone());
        script.on_aggro_range_enter(&mut ctx);
    }
}

/// `onSpellFinished`: a registered NPC's cast completed. The in-context
/// player is the cast's target when that target is a player (Java passes it
/// along); handlers that only touch the NPC work either way.
pub(crate) fn notify_spell_finished(
    world: &mut World,
    npc_oid: i32,
    npc_id: i32,
    skill_id: i32,
    target_oid: i32,
) {
    let registry = world.quests.clone();
    let scripts = registry.spell_finished_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let is_player_target = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .is_some();
    let (player, client_id) = if is_player_target {
        match client_for_player(world, target_oid) {
            Some(c) => (target_oid, c),
            None => (target_oid, 0),
        }
    } else {
        (0, 0)
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
        script.on_spell_finished(&mut ctx, skill_id);
    }
}

/// `Attackable` kill → registered kill quests' `onKill`. Called from
/// `death::npc_do_die` after combat rewards; killer-only (the
/// `getRandomPartyMemberState` party sharing is a documented deviation).
pub(crate) fn notify_kill(
    world: &mut World,
    killer_oid: i32,
    npc_oid: i32,
    npc_id: i32,
    is_summon: bool,
) {
    let registry = world.quests.clone();
    let scripts = registry.kill_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, killer_oid) else {
        return;
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, killer_oid, npc_oid, script.clone());
        // Java's `onKill(npc, killer, isSummon)` third argument — a handful of
        // scripts set the newly-spawned avenger on the *pet* that landed the
        // blow rather than on its owner.
        ctx.attack_is_summon = is_summon;
        script.on_kill(&mut ctx);
    }
}

/// The `onAttack` notification: a registered monster took damage from a
/// player (fired from `combat::npc_receive_damage`, killing blow included).
/// `player_oid` is the quest-acting player — the attacker itself, or a
/// servitor's owner. `skill_id` is the striking skill (`None` for melee) and
/// `is_summon` marks a servitor/pet blow, both surfaced to `on_attack` (Java's
/// `onAttack(npc, player, damage, isSummon, skill)`).
pub(crate) fn notify_attack(
    world: &mut World,
    player_oid: i32,
    npc_oid: i32,
    npc_id: i32,
    skill_id: Option<i32>,
    is_summon: bool,
) {
    let registry = world.quests.clone();
    let scripts = registry.attack_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, player_oid) else {
        return;
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, player_oid, npc_oid, script.clone());
        ctx.attack_skill_id = skill_id;
        ctx.attack_is_summon = is_summon;
        script.on_attack(&mut ctx);
    }
}

/// The `onSkillSee` notification: a registered NPC witnessed a skill cast by
/// `caster_oid`. Fired from the skill-finish path per affected NPC target
/// (quest 350's Soul Crystal absorb is a self-targeted read of the mob).
pub(crate) fn notify_skill_see(
    world: &mut World,
    caster_oid: i32,
    npc_oid: i32,
    npc_id: i32,
    skill_id: i32,
) {
    let registry = world.quests.clone();
    let scripts = registry.skill_see_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, caster_oid) else {
        return;
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, caster_oid, npc_oid, script.clone());
        script.on_skill_see(&mut ctx, skill_id);
    }
}

/// The `ON_PLAYER_LOGIN` notification (Java `Player.onPlayerEnter` →
/// `EventDispatcher`): fired at the end of the enter-world burst for every
/// global-event script. `npc` is 0.
pub(crate) fn notify_login(world: &mut World, client_id: u32, player: i32) {
    let registry = world.quests.clone();
    for script in registry.global_event_quests() {
        let mut ctx = QuestCtx::new(world, client_id, player, 0, script.clone());
        script.on_login(&mut ctx);
    }
}

/// The `ON_PLAYER_PRESS_TUTORIAL_MARK` notification
/// (`RequestTutorialQuestionMark` 0x87).
pub(crate) fn notify_tutorial_mark(world: &mut World, client_id: u32, player: i32, mark_id: i32) {
    let registry = world.quests.clone();
    for script in registry.global_event_quests() {
        let mut ctx = QuestCtx::new(world, client_id, player, 0, script.clone());
        script.on_tutorial_mark(&mut ctx, mark_id);
    }
}

/// The `ON_PLAYER_ITEM_PICKUP` notification (fired from
/// `ground_items::pickup_ground_item` after the give).
pub(crate) fn notify_item_pickup(world: &mut World, client_id: u32, player: i32, item_id: i32) {
    let registry = world.quests.clone();
    for script in registry.global_event_quests() {
        let mut ctx = QuestCtx::new(world, client_id, player, 0, script.clone());
        script.on_item_pickup(&mut ctx, item_id);
    }
}

/// The tutorial window's `bypass`/`link` press (`RequestTutorialPassCmdToServer`
/// 0x86 / `RequestTutorialLinkHtml` 0x85): `tutorial_close` closes the window
/// (Java's `TutorialClose` bypass handler), a `Quest <Name> <event>` command
/// fires the quest event with **no NPC** (this is Java's `OnPlayerBypass`
/// path — the tutorial window has no folk NPC behind it).
pub(crate) fn handle_tutorial_bypass(world: &mut World, client_id: u32, bypass: &str) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let bypass = bypass.trim();
    if bypass == "tutorial_close" {
        send_to_client(world, client_id, server_packets::tutorial_close_html());
        return;
    }
    if let Some(rest) = bypass.strip_prefix("Quest ") {
        let (name, event) = match rest.split_once(' ') {
            Some((n, e)) => (n, e.trim()),
            None => (rest, ""),
        };
        process_quest_event(world, client_id, player, 0, name, event);
    }
}

/// The `onSpawn` notification: a registered NPC just (re)spawned. No player
/// is involved — the ctx carries player/client 0 (see `QuestScript::on_spawn`).
/// Java `CreatureSeeTaskManager.run` — the once-per-second sweep behind
/// `addCreatureSeeId`. Every live watcher NPC scans the 3×3 region block
/// around it for creatures (players and NPCs) within its sight range — the
/// template's aggro range, or `AltPartyRange` when the template has none
/// (Java `initSeenCreatures`) — and fires `on_creature_see` once per newly
/// seen creature. The seen-set persists until the watcher despawns (a fresh
/// spawn starts blank), exactly like Java's per-creature `_seenCreatures`.
pub(crate) fn handle_creature_see_sweep(world: &mut World) {
    let registry = world.quests.clone();
    let mut watchers: Vec<(i32, i32, (i32, i32), crate::model::components::Position)> = Vec::new();
    world.objects.for_each_mut::<(
        &crate::model::npc::Npc,
        &crate::model::components::Position,
        &crate::model::components::Vitals,
        &crate::model::components::RegionCell,
    )>(|(n, p, v, r)| {
        if !v.dead && registry.has_creature_see(n.npc_id) {
            watchers.push((n.object_id, n.npc_id, r.0, *p));
        }
    });
    for (npc_oid, npc_id, region, pos) in watchers {
        let range = {
            let aggro = world.data.npc_data.get(npc_id).map_or(0, |t| t.aggro_range);
            f64::from(if aggro > 0 {
                aggro
            } else {
                world.cfg.character.alt_party_range
            })
        };
        let instance = crate::game_loop::helpers::instance_of(world, npc_oid);
        let in_sight = |world: &World, oid: i32| {
            if crate::game_loop::helpers::instance_of(world, oid) != instance {
                return false;
            }
            if is_dead(world, oid) {
                return false;
            }
            crate::geo::distance::within_3d_xyz(world, oid, pos.x, pos.y, pos.z, range)
        };
        let mut fresh: Vec<i32> = Vec::new();
        // Players in the surrounding block (Java skips invisible ones).
        for pid in world.players_visible_from(region).collect::<Vec<_>>() {
            let hidden = world
                .objects
                .get_component::<crate::model::components::AdminFlags>(&pid)
                .is_some_and(|f| f.hidden);
            if !hidden && in_sight(world, pid) {
                fresh.push(pid);
            }
        }
        // NPCs in the surrounding block.
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(list) = world.npc_regions.get(&(region.0 + dx, region.1 + dy)) {
                    for &noid in list {
                        if noid != npc_oid && in_sight(world, noid) {
                            fresh.push(noid);
                        }
                    }
                }
            }
        }
        if world
            .objects
            .get_component::<crate::model::components::SeenCreatures>(&npc_oid)
            .is_none()
        {
            world
                .objects
                .add_components(&npc_oid, crate::model::components::SeenCreatures::default());
        }
        let newly: Vec<i32> = {
            let Some(seen) = world
                .objects
                .get_component_mut::<crate::model::components::SeenCreatures>(&npc_oid)
            else {
                continue;
            };
            fresh.into_iter().filter(|&c| seen.0.insert(c)).collect()
        };
        for creature in newly {
            let is_player = world
                .objects
                .has_component::<crate::model::Player>(&creature);
            let (player, client_id) = if is_player {
                (creature, client_for_player(world, creature).unwrap_or(0))
            } else {
                (0, 0)
            };
            for script in registry.creature_see_quests(npc_id) {
                let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
                script.on_creature_see(&mut ctx, creature);
            }
        }
    }
    world
        .scheduler
        .schedule(world.tick + 10, ScheduledTask::CreatureSeeSweep);
}

pub(crate) fn notify_spawn(world: &mut World, npc_oid: i32, npc_id: i32) {
    let registry = world.quests.clone();
    let scripts = registry.spawn_quests(npc_id);
    for script in scripts {
        let mut ctx = QuestCtx::new(world, 0, 0, npc_oid, script.clone());
        script.on_spawn(&mut ctx);
    }
}

/// `ScheduledTask::QuestTimer` firing: seq-check against `QuestTimerSeqs`,
/// then `on_timer`.
pub(crate) fn handle_quest_timer(
    world: &mut World,
    quest: &'static str,
    name: &str,
    player: i32,
    npc: i32,
    seq: u64,
) {
    let live = world
        .objects
        .get_component::<QuestTimerSeqs>(&player)
        .and_then(|t| t.0.get(&(quest, name.to_string())).copied());
    if live != Some(seq) {
        return; // cancelled or superseded
    }
    if let Some(t) = world.objects.get_component_mut::<QuestTimerSeqs>(&player) {
        t.0.remove(&(quest, name.to_string()));
    }
    let Some(client_id) = client_for_player(world, player) else {
        return;
    };
    let registry = world.quests.clone();
    let Some(script) = registry.by_name(quest) else {
        return;
    };
    let mut ctx = QuestCtx::new(world, client_id, player, npc, script.clone());
    script.on_timer(&mut ctx, name);
}

/// `RequestQuestAbort` (0x63): the quest UI's Abandon button —
/// `qs.exitQuest(true)` + `QuestList`, no sound.
/// `RequestQuestList` (0x62, G33): the client opened its quest journal — resend
/// the `QuestList` (Java `player.sendPacket(new QuestList(player))`). Empty body.
pub(crate) fn handle_request_quest_list(world: &World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(quests) = world.objects.get_component::<Quests>(&player) else {
        return;
    };
    let pkt = ew::quest_list(quests, &world.quests);
    send_to_client(world, client_id, pkt);
}

pub(crate) fn handle_request_quest_abort(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = crate::network::client_packets::RequestQuestAbort::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let registry = world.quests.clone();
    let Some(script) = registry.by_id(pkt.quest_id) else {
        return;
    };
    let mut ctx = QuestCtx::new(world, client_id, player, 0, script);
    if ctx.is_started() {
        ctx.exit_quest(true, false);
    }
}
