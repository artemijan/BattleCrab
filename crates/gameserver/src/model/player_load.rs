//! Building a `PlayerData` from the DB row and putting it into the world —
//! `Player::from_char` and the restore steps that feed it.

use crate::data::GameData;
use crate::db::CharData;

use crate::game_loop;

use super::components::{
    AttackState, BaseStats, Buffs, ClientPos, Collision, CombatStats, Macros, PlayerVitals,
    Position, RegionCell, Reuses, Shortcuts, SkillBook, Speeds, StatModifiers, TargetRef, Vitals,
};
use super::equip_conditions::conditioned_passive_buffs;
use super::inventory::{self, Inventory};
use super::max_vitals::{calc_max_cp, calc_max_hp, calc_max_mp, hp_percent_of};
use super::{DEFAULT_NAME_COLOR, DEFAULT_TITLE_COLOR, Player, PlayerData, SkillReuse};
use super::{components, shortcut};

/// Java `Player.restoreSkills`' skill check, factored out of
/// [`Player::from_char`] so it can be read (and tested) as the one thing it is.
///
/// Java's guard is
/// `SKILL_CHECK_ENABLE && (!canOverrideCond(SKILL_CONDITIONS) || SKILL_CHECK_GM)
/// && !isSkillAllowed(...)`, evaluated per restored row. The middle clause is
/// the one worth reading twice: with `SkillCheckGM = False` — this dist — a
/// character holding the `SKILL_CONDITIONS` override is **skipped entirely**,
/// so the key named "check GMs" turns checking GMs off.
///
/// Returns the book to keep and everything that failed. Failures are reported
/// whether or not `SkillCheckRemove` takes them out: an operator running the
/// check as a pure audit still wants the list.
fn check_restored_skills(
    data: &GameData,
    c: &CharData,
    cond_overrides: u64,
    skills: SkillBook,
) -> (SkillBook, Vec<(i32, i32)>) {
    if !data.skill_check.enable {
        return (skills, Vec::new());
    }
    // `canOverrideCond(PlayerCondOverride.SKILL_CONDITIONS)`.
    let overrides_skill_conditions =
        cond_overrides & (1u64 << crate::game_loop::admin::SKILL_CONDITIONS_ORDINAL) != 0;
    if overrides_skill_conditions && !data.skill_check.gm {
        return (skills, Vec::new());
    }
    let is_gm = data.admin.is_gm(
        data.admin
            .effective_access_level(c.access_level, data.default_access_level),
    );
    let race = crate::enums::Race::from_ordinal(c.race);
    let mut illegal: Vec<(i32, i32)> = Vec::new();
    let mut kept = SkillBook::default();
    for (&id, &level) in &skills.0 {
        // Java's first arm, `skill.isExcludedFromCheck()`, reads the *skill*
        // rather than any tree — the datapack's own opt-out for skills learned
        // by routes that are not class trees (the subclass certifications).
        let excluded = data
            .skill_data
            .get(id, level)
            .is_some_and(|s| s.excluded_from_check);
        let max_level = data.skill_data.max_level(id);
        if excluded
            || data
                .skill_trees
                .is_skill_allowed(c.class_id, race, is_gm, id, level, max_level)
        {
            kept.0.insert(id, level);
            continue;
        }
        illegal.push((id, level));
    }
    illegal.sort_unstable();
    // `Config.SKILL_CHECK_REMOVE` — off, the check is an audit and the book is
    // returned untouched.
    if data.skill_check.remove {
        (kept, illegal)
    } else {
        (skills, illegal)
    }
}

impl PlayerData {
    /// Java `restoreEffects` (skill-reuse half): rebuild the live cooldown map
    /// from the `character_skills_save` rows the DB loaded. Each row's absolute
    /// `systime_ms` becomes an `until_tick` off the current game tick, using the
    /// real remaining time (`systime - now`), so a cooldown decays across the
    /// offline gap. Rows already expired at restore are skipped.
    pub fn restore_reuses(&mut self, c: &CharData, now_tick: u64, now_wallclock_ms: i64) {
        for r in &c.skill_reuses {
            let remaining_ms = r.systime_ms - now_wallclock_ms;
            if remaining_ms <= 0 {
                continue;
            }
            self.reuses.0.insert(
                r.reuse_key,
                SkillReuse {
                    skill_level: r.skill_level,
                    until_tick: now_tick + crate::scheduler::ms_to_ticks(remaining_ms),
                    total_ms: r.reuse_delay,
                },
            );
        }
    }

    /// Java `restoreEffects` (buff half), staging step: carry the loaded buff
    /// rows on the bundle so enter-world can re-apply them after the character
    /// spawns. No time arithmetic happens here — unlike a cooldown, a buff's
    /// stored `remaining_time` is relative and its countdown does not advance
    /// while the character is offline, so the value is used verbatim.
    pub fn restore_buffs(&mut self, c: &CharData) {
        self.pending_buffs = c.skill_buffs.clone();
    }

    /// Spawn into the world registry (Java `World.addObject` at EnterWorld).
    ///
    /// Takes the whole `World`, not just the store, so the new player lands in
    /// `World.player_regions` in the same step it lands in the ECS — an entity
    /// spawned without its index entry is invisible to every broadcast.
    pub fn spawn_into(self, world: &mut crate::world::World) {
        let object_id = self.player.object_id;
        let region = self.region.0;
        world.objects.spawn(
            self.player.object_id,
            (
                self.player,
                (
                    self.position,
                    self.region,
                    self.vitals,
                    self.player_vitals,
                    self.base_stats,
                    self.speeds,
                    self.collision,
                    self.combat,
                ),
                (
                    self.inventory,
                    self.skills,
                    self.shortcuts,
                    self.macros,
                    self.friends,
                    self.quests,
                    AttackState::default(),
                    TargetRef::default(),
                    ClientPos::default(),
                    self.buffs,
                    self.stat_modifiers,
                    self.reuses,
                    components::ZoneFlags::default(),
                    components::ExpertisePenalty::default(),
                    components::PvpState::default(),
                ),
                (
                    self.warehouse,
                    self.freight,
                    components::ClanSkills::default(),
                    components::OptionSkills::default(),
                    components::OptionTriggers::default(),
                    self.skill_enchants,
                    self.henna,
                    self.recipe_book,
                    self.variables,
                    self.pets,
                    self.pet_inventory,
                    self.summons,
                ),
            ),
        );
        world.index_player(object_id, region);
    }
}

impl Player {
    /// Build a `Player` (+ its extracted components, as a `PlayerData`)
    /// from a stored character row + its class template.
    /// Max HP/MP/CP are recomputed (not read from the DB) so they display
    /// correctly; current HP/MP/CP come from the row, clamped to the max.
    pub fn from_char(data: &GameData, c: &CharData) -> PlayerData {
        // Java `Player.setAccessLevel`, called from `restore`: `DefaultAccessLevel`
        // promotes a level-0 character. 0 on this dist, so it is the identity —
        // but every access-level read below has to see the promoted value, not
        // the stored one, or an operator who sets the key gets GMs whose
        // condition overrides and skill-check exemption do not match their tier.
        let access_level = data
            .admin
            .effective_access_level(c.access_level, data.default_access_level);
        let cond_overrides = if data.admin.is_gm(access_level) {
            crate::game_loop::admin::all_exceptions_mask()
        } else {
            0
        };
        // The active class's template (base classes only in G4).
        let t = data
            .player_templates
            .get_or_base(c.class_id, c.base_class_id)
            .cloned()
            .unwrap_or_default();

        // Split stored items by location: warehouse / freight rows go to their
        // own containers, everything else (inventory + paperdoll) to inventory.
        let (wh_rows, rest): (Vec<_>, Vec<_>) =
            c.items.iter().cloned().partition(|r| r.loc == "WAREHOUSE");
        let (freight_rows, rest): (Vec<_>, Vec<_>) =
            rest.into_iter().partition(|r| r.loc == "FREIGHT");
        // Pet-held items (Java `ItemLocation.PET`) are stored against the
        // *player's* owner id, so they arrive in the same batch.
        let (pet_rows, inv_rows): (Vec<_>, Vec<_>) = rest
            .into_iter()
            .partition(|r| r.loc == "PET" || r.loc == "PET_EQUIP");
        let warehouse = inventory::Warehouse::from_rows(&wh_rows);
        let freight = inventory::Freight::from_rows(&freight_rows);
        let pet_inventory = inventory::PetInventory::from_rows(&pet_rows);

        // Built early so equipped gear feeds every finalizer below — max HP/MP
        // (item +MP jewelry) as well as the combat recompute further down.
        let inventory = Inventory::from_rows(&inv_rows);
        // No buffs are active at load; the enter-world clan-skill / passive
        // pass recomputes these through `recompute_max_vitals` once buffs land.
        let no_mods = StatModifiers::default();
        let max_hp = calc_max_hp(data, &t, c.level, Some(&inventory), &no_mods);
        let max_mp = calc_max_mp(data, &t, c.level, Some(&inventory), &no_mods);
        let max_cp = calc_max_cp(data, &t, c.level, &no_mods);

        // Restored henna dyes (Java `restoreHenna`): their base-stat bonuses are
        // folded straight into `BaseStats` — henna is a permanent base modifier,
        // exactly like the template, so every downstream reader (finalizers,
        // UserInfo STR/…) picks it up with no special-casing.
        let mut henna_slots = [None; 3];
        for &(slot, dye_id) in &c.hennas {
            if (1..=3).contains(&slot) {
                henna_slots[(slot - 1) as usize] = Some(dye_id);
            }
        }
        let henna = components::HennaSlots(henna_slots);
        let hs = data.hennas.stat_sums(&henna_slots);
        // A complete worn armor set adds flat base stats exactly as a dye does
        // (Java `BaseStatFinalizer`). Folded in here so the enter-world
        // `UserInfo` already carries them; `compose_base_stats` is the same
        // sum for every later recompute.
        let sets = game_loop::items::armor_sets::set_stat_sums_for(&data.armor_sets, &inventory);
        let base_stats = BaseStats {
            str_: t.base_str + hs.str_ + sets.str_ as i32,
            dex: t.base_dex + hs.dex + sets.dex as i32,
            con: t.base_con + hs.con + sets.con as i32,
            int_: t.base_int + hs.int_ + sets.int_ as i32,
            wit: t.base_wit + hs.wit + sets.wit as i32,
            men: t.base_men + hs.men + sets.men as i32,
        };
        let mut vitals = Vitals {
            max_hp: max_hp as i32,
            cur_hp: c.cur_hp.min(max_hp),
            max_mp: max_mp as i32,
            cur_mp: c.cur_mp.min(max_mp),
            dead: c.cur_hp < 0.5,
        };
        // Java `Player.restore`: `setCurrentCp(currentCp)` replays the stored
        // `curCp`, clamped to the freshly computed max — the same treatment
        // `curHp`/`curMp` get just above.
        let mut player_vitals = PlayerVitals {
            max_cp: max_cp as i32,
            cur_cp: c.cur_cp.min(max_cp),
        };
        let mut speeds = Speeds {
            run_spd: t.base_run_spd as f64,
            walk_spd: t.base_walk_spd as f64,
            swim_run_spd: t.base_swim_run_spd as f64,
            swim_walk_spd: t.base_swim_walk_spd as f64,
            move_multiplier: 1.0,
            base_run_spd: t.base_run_spd as f64,
            base_walk_spd: t.base_walk_spd as f64,
            base_swim_run_spd: t.base_swim_run_spd as f64,
            base_swim_walk_spd: t.base_swim_walk_spd as f64,
            running: true,
            swimming: false,
            swamp_multiplier: 1.0,
        };
        // Java `PlayerTemplate.getCollisionRadius()` picks the box by
        // `appearance.isFemale()`; the two differ for every class on this dist.
        let (radius, height) = t.collision(c.sex != 0);
        let collision = Collision { radius, height };
        // Java `setAccessLevel` folds the tier's name/title color into the
        // appearance; a level-0 player keeps the client defaults (see
        // `Player::name_color`).
        let access = data.admin.access_level(c.access_level);
        let (name_color, title_color) = if c.access_level != 0 {
            (access.name_color, access.title_color)
        } else {
            (DEFAULT_NAME_COLOR, DEFAULT_TITLE_COLOR)
        };
        // Java `CharInfo`/`UserInfo`: hero glow = `isHero() || (isGM() &&
        // GM_HERO_AURA)`. `isHero()` is set by the olympiad's crowning (G25)
        // and by `//sethero`; either recomputes this.
        let hero_aura = access.is_gm && data.gm.hero_aura;
        let p = Player {
            object_id: c.object_id,
            name: c.name.clone(),
            account: c.account_name.clone(),
            title: String::new(),
            access_level,
            name_color,
            title_color,
            hero_aura,
            is_noble: c.noble,
            class_index: c
                .subclasses
                .iter()
                .find(|s| s.class_id == c.class_id)
                .map(|s| s.class_index)
                .unwrap_or(0),
            subclasses: c.subclasses.clone(),
            skills_by_index: c.skills_by_index.clone(),
            team: 0,
            on_event: false,
            registered_on_event: false,
            hennas_by_index: c.hennas_by_index.clone(),
            shortcuts_by_index: c.shortcuts_by_index.clone(),
            base_level: c.level,
            base_exp: c.exp,
            base_sp: c.sp,
            is_hero: false,
            true_hero: false,
            tele_mode: crate::enums::AdminTeleportType::Normal,
            blink_active: false,
            falling_until_tick: 0,
            level: c.level,
            class_id: c.class_id,
            base_class_id: c.base_class_id,
            race: c.race,
            is_female: c.sex != 0,
            exp: c.exp,
            sp: c.sp,
            reputation: c.reputation,
            pk_kills: c.pk_kills,
            raidboss_points: c.raidboss_points,
            pvp_kills: c.pvp_kills,
            // A fresh session starts unowned; `cursed_weapon::on_enter_world`
            // (G28) restores it for a player who logged out still cursed.
            cursed_weapon_equipped_id: 0,
            charges: 0,
            charges_seq: 0,
            vitality_points: c.vitality_points,
            pccafe_points: c.pccafe_points,
            prime_points: c.prime_points,
            fame: 0,
            // `character_reco_bonus` row (Java `Player.loadRecommendations`).
            // A new character's row is seeded with rec_left=20 at creation
            // (`Player.create` → `setRecomLeft(20)`); `db::load_reco_bonus`
            // returns those two values (or 0/0 when the row is absent).
            rec_have: c.rec_have,
            rec_left: c.rec_left,
            reco_two_hours_given: false,
            reco_give_seq: 0,
            pc_cafe_seq: 0,
            clan_id: c.clan_id,
            clan_privs: c.clan_privs,
            clan_leader: false, // fixed up at enter-world from World.clans
            pledge_class: 0,    // recomputed with clan_leader from World.clans
            clan_create_expiry_time: c.clan_create_expiry_time,
            clan_join_expiry_time: c.clan_join_expiry_time,
            create_date: c.create_date.clone(),
            power_grade: c.power_grade,
            ally_id: 0, // synced from the clan at enter-world
            siege_state: 0,
            siege_side: 0,
            pledge_type: c.pledge_type,
            lvl_joined_academy: c.lvl_joined_academy,
            apprentice: c.apprentice,
            sponsor: c.sponsor,
            clan_crest_id: 0, // synced from the clan at enter-world
            clan_crest_large_id: 0,
            ally_crest_id: 0, // synced from the clan at enter-world
            face: c.face,
            hair_style: c.hair_style,
            hair_color: c.hair_color,
            cast_seq: 0,
            pending_revive: false,
            lost_exp_on_death: c.lost_exp_on_death,
            revive_request: None,
            summon_request: None,
            pending_pet_collar: None,
            pending_mercenary_ticket: None,
            teleporting: false,
            jailed: false,
            sitting: false,
            selling_buffs: false,
            sell_buff_list: Vec::new(),
            last_petition_gm_name: None,
            snoop_listeners: Vec::new(),
            snooped: Vec::new(),
            gm_hidden: false,
            quest_zone_id: -1,
            charged_shots: 0,
            auto_shots: Vec::new(),
            mount_type: 0,
            mount_npc_id: 0,
            mount_level: 0,
            mount_feed: 0,
            mount_collar_object_id: 0,
            char_info_pending: false,
            trade_refusal: false,
            // Java `Player.restore`: `if (player.isGM())
            // setOverrideCond(variables.getLong(COND_OVERRIDE_KEY,
            // PlayerCondOverride.getAllExceptionsMask()))` — a GM who has never
            // touched `//set_exception` overrides **everything** by default.
            // The port used to start every character at 0, which left
            // `//exceptions` showing a GM as overriding nothing and made
            // `SkillCheckGM` unreachable (nothing ever held the override at
            // load, so the key it gates could not matter). The variable itself
            // is still not persisted here, so this is the default arm only.
            cond_overrides: if data.admin.is_gm(c.access_level) {
                crate::game_loop::admin::all_exceptions_mask()
            } else {
                0
            },
            transform_id: 0,
            transform_display_id: 0,
            store_type: 0,
            spawn_protect_end_tick: 0,
        };
        // Filled in by `recalculate_stats` (incl. atk_range/random_dmg, which it
        // sets from the equipped weapon or the class template).
        let mut combat = CombatStats::default();
        let mut mods = StatModifiers::default();
        let mut buffs = Buffs::default();
        p.recalculate_stats(
            data,
            &base_stats,
            &mods,
            &inventory,
            &mut speeds,
            &mut combat,
        );
        // Java `restoreCharData` → `addSkill`: fold the character's known
        // armor-conditioned passives (Spellcraft/Magician's Movement) into the
        // stat maps now, so the enter-world `UserInfo` burst already carries them
        // (no separate post-spawn resend). Timed buffs aren't restored yet.
        // Transform-granted skills are session-only and are filtered out of
        // every flush (`net::build_save_data`), but rows written before that
        // filter existed can still be in the DB — drop them here too, since a
        // fresh login is never transformed (Dissonance 5437's Accuracy -50
        // otherwise follows the character across relogs).
        let skills = SkillBook(
            c.skills
                .iter()
                // Armor-set skills are session-only for the same reason and
                // are dropped here too; the worn set re-grants them a few lines
                // below, so a character who logs in wearing one keeps the bonus
                // while one who logged out and sold the set does not.
                .filter(|&&(id, _, _)| {
                    !data.transforms.is_transform_skill(id)
                        && !data.armor_sets.is_armor_set_skill(id)
                })
                .map(|&(id, lvl, _)| (id, lvl))
                .collect(),
        );
        // Java `restoreSkills`' skill check. It runs over the rows read **from
        // the database**, which is the whole reason it sits here and not after
        // the derived grants below: Java iterates its `ResultSet`, not the
        // finished `_skills` map, so a skill that is granted rather than stored
        // is never a candidate. Check the book instead and the armour-set and
        // noble grants — which are in no allow-list arm, correctly — get eaten
        // the moment they are added.
        let (skills, illegal_skills) = check_restored_skills(data, c, cond_overrides, skills);

        // Re-grant whatever the gear the character logged out wearing entitles
        // them to. The rows themselves were just filtered out above, so this is
        // the only thing that puts a set bonus back — without it a relog would
        // silently drop every armor-set passive, which is precisely how the
        // augment options regressed before they were re-derived here too.
        let mut skills = skills;
        for (id, level) in
            game_loop::items::armor_sets::granted_skills_for(&data.armor_sets, &inventory)
        {
            skills.0.insert(id, level);
        }
        // Java `Player.restore`: `player.setNoble(rset.getInt("nobless") == 1)`,
        // whose `setNoble(true)` grants the noble tree with
        // `addSkill(skill, false)` — **granted from the column, never
        // persisted**. The port had it the other way round: nothing re-granted
        // at load and the skills survived only as `character_skills` rows, so a
        // nobless who was stripped of it kept every skill, and the rows are
        // exactly what `is_skill_allowed` is built to reject. Deriving them
        // here is what lets the check remove the rows without taking the
        // skills with them.
        if c.noble {
            for &(id, level) in data.skill_trees.noble_skills() {
                skills.0.insert(id, level);
            }
        }

        // The enchant sub-levels ride the same rows (PLAN_G19_SKILL_ENCHANT.md).
        let skill_enchants = components::SkillEnchants(
            c.skills
                .iter()
                .filter(|&&(_, _, sub)| sub > 0)
                .map(|&(id, _, sub)| (id, sub))
                .collect(),
        );
        // Java `restoreRecipeBook`: classify each stored recipe-list id into the
        // dwarven/common book by its `RecipeList.isDwarvenRecipe()`; ids with no
        // matching recipe are dropped (Java's `recipe == null` continue).
        let mut recipe_book = components::RecipeBook::default();
        for &list_id in &c.recipe_book {
            match data.recipes.get(list_id) {
                Some(r) if r.is_dwarven => recipe_book.dwarven.push(list_id),
                Some(_) => recipe_book.common.push(list_id),
                None => {}
            }
        }
        // The HP-conditioned passives (Final Frenzy 290, Final Fortress 291)
        // are evaluated against the *stored* HP, so a character who logs out
        // below 30 % logs back in with the bonus already up — which is what
        // Java's first `recalculateStats` after `restore` does.
        for buff in conditioned_passive_buffs(
            data,
            &skills,
            &inventory,
            hp_percent_of(vitals.cur_hp, vitals.max_hp),
        ) {
            p.apply_buff(
                data,
                &base_stats,
                &mut mods,
                &inventory,
                &mut buffs,
                &mut speeds,
                &mut combat,
                buff,
            );
        }
        // Those passive skills can carry MaxHp/MaxMp/MaxCp modifiers (e.g. a
        // mystic's MP passives, which drive most of an Archmage's MP pool). They
        // land in `mods` above, but the vitals were computed before the passive
        // pass — recompute them now so the enter-world `UserInfo` carries the
        // boosted maxima. Java's Max{Hp,Mp,Cp}Finalizer run inside the same
        // `recalculateStats`; keep current values (clamp only on shrink).
        vitals.max_hp = calc_max_hp(data, &t, c.level, Some(&inventory), &mods) as i32;
        vitals.max_mp = calc_max_mp(data, &t, c.level, Some(&inventory), &mods) as i32;
        player_vitals.max_cp = calc_max_cp(data, &t, c.level, &mods) as i32;
        vitals.cur_hp = vitals.cur_hp.min(vitals.max_hp as f64);
        vitals.cur_mp = vitals.cur_mp.min(vitals.max_mp as f64);
        player_vitals.cur_cp = player_vitals.cur_cp.min(player_vitals.max_cp as f64);

        // `ShortCuts.restoreMe`'s verification tail: ITEM shortcuts whose
        // object id left the inventory are dropped here, so they never reach the
        // bundle and the next persistence flush's reconcile removes their rows
        // (memory-first — no per-select `DeleteShortcut`; see
        // `stale_item_shortcuts`). Surviving *EtcItem* shortcuts pick up the
        // template's shared reuse group (weapons/armor keep -1 on restore — a
        // Java quirk kept as-is).
        let shortcuts = c
            .shortcuts
            .iter()
            .filter(|sc| {
                sc.kind != shortcut::ShortcutType::Item
                    || c.items.iter().any(|i| i.object_id == sc.id)
            })
            .map(|sc| {
                let mut sc = *sc;
                if sc.kind == shortcut::ShortcutType::Item {
                    let is_etc = c
                        .items
                        .iter()
                        .find(|i| i.object_id == sc.id)
                        .and_then(|i| data.item_data.get(i.item_id))
                        .is_some_and(|t| t.kind == crate::data::item_data::ItemKind::Etc);
                    if is_etc {
                        // `shared_reuse_group` template default (never set in
                        // this dist's item XMLs).
                        sc.shared_reuse_group = 0;
                    }
                }
                sc
            })
            .collect();

        PlayerData {
            player: p,
            position: Position {
                x: c.x,
                y: c.y,
                z: c.z,
                heading: 0,
            },
            region: RegionCell(crate::world::region_of(c.x, c.y)),
            vitals,
            player_vitals,
            base_stats,
            speeds,
            collision,
            combat,
            buffs,
            stat_modifiers: mods,
            inventory,
            warehouse,
            freight,
            skills,
            skill_enchants,
            henna,
            recipe_book,
            variables: components::PlayerVariables(c.variables.iter().cloned().collect()),
            pets: components::PlayerPets(
                c.pets
                    .iter()
                    .map(|p| (p.collar_object_id, p.clone()))
                    .collect(),
            ),
            summons: components::PlayerSummons(c.summons.clone()),
            pet_inventory,
            shortcuts: Shortcuts::from_list(shortcuts),
            macros: Macros::from_list(c.macros.clone()),
            friends: components::Friends(c.friends.clone()),
            quests: components::Quests(c.quests.clone()),
            // Filled by the select path via `restore_reuses` (needs the game
            // tick); empty here keeps the many test callers unchanged.
            reuses: Reuses::default(),
            // Likewise filled by the select path, via `restore_buffs`.
            pending_buffs: Vec::new(),
            illegal_skills,
        }
    }

    /// The ITEM shortcuts `from_char` will prune (object id no longer in the
    /// inventory) — the character-select handler deletes their DB rows, the
    /// `deleteShortCutFromDb` half of `ShortCuts.restoreMe`'s verification.
    pub fn stale_item_shortcuts(c: &CharData) -> Vec<(i32, i32)> {
        c.shortcuts
            .iter()
            .filter(|sc| {
                sc.kind == shortcut::ShortcutType::Item
                    && !c.items.iter().any(|i| i.object_id == sc.id)
            })
            .map(|sc| (sc.slot, sc.page))
            .collect()
    }
}
