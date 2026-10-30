//! The compiled-in scripts — the Rust counterpart of
//! `dist/game/data/scripts/**.java` (which Java compiles at boot; here each
//! script is a module registering a `QuestScript` trait object in
//! [`build_registry`]). Framework in `game_loop/quests.rs`; this module is
//! only the content.

pub mod alliance;

/// The 60 village masters that the clan and alliance dialogs both attach to —
/// Java's identical `NPCS` array in `ClanMaster` and `AllianceMaster`.
///
/// One list, because it is one fact: these are *the village masters*. Both
/// scripts serve every one of them from a single page set numbered against a
/// virtual npc id (`9000-NN` / `9001-NN`), so a master present in one list and
/// absent from the other would answer one dialog and silently ignore the other.
pub(crate) const VILLAGE_MASTERS: [i32; 60] = [
    30026, 30031, 30037, 30066, 30070, 30109, 30115, 30120, 30154, 30174, //
    30175, 30176, 30187, 30191, 30195, 30288, 30289, 30290, 30297, 30358, //
    30373, 30462, 30474, 30498, 30499, 30500, 30503, 30504, 30505, 30508, //
    30511, 30512, 30513, 30520, 30525, 30565, 30594, 30595, 30676, 30677, //
    30681, 30685, 30687, 30689, 30694, 30699, 30704, 30845, 30847, 30849, //
    30854, 30857, 30862, 30865, 30894, 30897, 30900, 30905, 30910, 30913,
];
pub mod alliance_master;
pub mod antharas_heart;
pub mod awakening_master;
pub mod baby_pets;
pub mod baium;
pub mod castle_chamberlain;
mod castle_services;
mod cave_maiden;
mod chamber_of_prophecies;
pub mod clan_hall_auctioneer;
pub mod clan_hall_door_manager;
pub mod clan_hall_manager;
pub mod clan_master;
mod den_of_evil;
mod dr_chaos_talk;
mod elroki_teleporters;
mod evil_incubator;
pub mod feedable_beasts;
mod forge_of_the_gods;
mod fortress_of_the_dead;
pub mod four_sepulchers;
mod frozen_labyrinth;
mod harnak_underground_ruins;
mod hot_springs;
pub mod labyrinth_of_belis;
pub mod last_imperial_tomb;
mod mammons;
mod misc_npcs;
mod mob_behaviours;
pub mod monument_of_heroes;
mod mysterious_wizard;
pub mod newbie_chain;
pub mod newbie_guide;
pub mod nobless_master;
pub mod npc_location_info;
pub mod oly_buffer;
mod oly_manager;
pub mod orc_amulet_hunt;
mod pagan_keys;
mod pagan_teleporters;
mod plains_of_dion;
pub mod primeval_isle;
mod pytan;
pub mod q00032_an_obvious_lie;
pub mod q00033_make_a_pair_of_dress_shoes;
pub mod q00034_in_search_of_cloth;
pub mod q00035_find_glittering_jewelry;
pub mod q00036_make_a_sewing_kit;
pub mod q00037_make_formal_wear;
pub mod q00042_help_the_uncle;
pub mod q00043_help_the_sister;
pub mod q00044_help_the_son;
pub mod q00109_in_search_of_the_nest;
pub mod q00110_to_the_primeval_isle;
pub mod q00111_elrokian_hunters_proof;
pub mod q00124_meeting_the_elroki;
pub mod q00125_the_name_of_evil_1;
pub mod q00126_the_name_of_evil_2;
pub mod q00127_fishing_specialists_request;
pub mod q00210_obtain_a_wolf_pet;
pub mod q00257_the_guard_is_busy;
pub mod q00258_bring_wolf_pelts;
pub mod q00259_request_from_the_farm_owner;
pub mod q00260_orc_hunting;
pub mod q00261_collectors_dream;
pub mod q00262_trade_with_the_ivory_tower;
pub mod q00263_orc_subjugation;
pub mod q00264_keen_claws;
pub mod q00265_bonds_of_slavery;
pub mod q00266_pleas_of_pixies;
pub mod q00267_wrath_of_verdure;
pub mod q00271_proof_of_valor;
pub mod q00272_wrath_of_ancestors;
pub mod q00273_invaders_of_the_holy_land;
pub mod q00274_skirmish_with_the_werewolves;
pub mod q00275_dark_winged_spies;
pub mod q00276_totem_of_the_hestui;
pub mod q00277_gatekeepers_offering;
pub mod q00292_brigands_sweep;
pub mod q00293_the_hidden_veins;
pub mod q00294_covert_business;
pub mod q00295_dreaming_of_the_skies;
pub mod q00296_tarantulas_spider_silk;
pub mod q00297_gatekeepers_favor;
pub mod q00300_hunting_leto_lizardman;
pub mod q00303_collect_arrowheads;
pub mod q00306_crystal_of_fire_and_ice;
pub mod q00313_collect_spores;
pub mod q00316_destroy_plague_carriers;
pub mod q00317_catch_the_wind;
pub mod q00319_scent_of_death;
pub mod q00320_bones_tell_the_future;
pub mod q00324_sweetest_venom;
pub mod q00325_grim_collector;
pub mod q00326_vanquish_remnants;
pub mod q00327_recover_the_farmland;
pub mod q00328_sense_for_business;
pub mod q00329_curiosity_of_a_dwarf;
pub mod q00331_arrow_of_vengeance;
pub mod q00333_hunt_of_the_black_lion;
pub mod q00344_1000_years_the_end_of_lamentation;
pub mod q00348_an_arrogant_search;
pub mod q00354_conquest_of_alligator_island;
pub mod q00355_family_honor;
pub mod q00356_dig_up_the_sea_of_spores;
pub mod q00358_illegitimate_child_of_the_goddess;
pub mod q00360_plunder_their_supplies;
pub mod q00369_collector_of_jewels;
pub mod q00370_an_elder_sows_seeds;
pub mod q00373_supplier_of_reagents;
pub mod q00374_whisper_of_dreams_part1;
pub mod q00375_whisper_of_dreams_part2;
pub mod q00420_little_wing;
pub mod q00421_little_wings_big_adventure;
pub mod q00605_alliance_with_ketra_orcs;
pub mod q00606_battle_against_varka_silenos;
pub mod q00611_alliance_with_varka_silenos;
pub mod q00612_battle_against_ketra_orcs;
pub mod q00617_gather_the_flames;
pub mod q00619_relics_of_the_old_empire;
pub mod q00620_four_goblets;
pub mod q00622_specialty_liquor_delivery;
pub mod q00623_the_finest_food;
pub mod q00628_hunt_golden_ram;
pub mod q00634_in_search_of_fragments_of_dimension;
pub mod q00640_the_zero_hour;
pub mod q00641_attack_sailren;
pub mod q00642_a_powerful_primeval_creature;
pub mod q00643_rise_and_fall_of_the_elroki_tribe;
pub mod q00662_a_game_of_cards;
pub mod q00688_defeat_the_elrokian_raiders;
pub mod q10331_start_of_fate;
pub mod q10338_seize_your_destiny;
pub mod q10360_certification_of_fate;
pub mod q10751_winds_of_fate_encounters;
pub mod q10752_winds_of_fate_a_promise;
pub mod q10753_winds_of_fate_choices;
pub mod q10866_punitive_operation_on_the_devil_isle;
pub mod q10993_future_dwarves;
pub mod q10994_future_orcs;
pub mod q10995_mutual_benefit;
pub mod q10996_temples_decorations;
pub mod q10997_loser_priest1;
pub mod q10998_loser_priest2;
pub mod q10999_loser_priest3;
pub mod q11000_moon_knight;
pub mod q11001_tombs_of_ancestors;
pub mod q11002_help_with_temple_restoration;
pub mod q11003_perfect_leather_armor1;
pub mod q11004_perfect_leather_armor2;
pub mod q11005_perfect_leather_armor3;
pub mod q11006_future_people;
pub mod q11007_noise_in_woods;
pub mod q11008_preparation_for_dungeon;
pub mod q11009_new_potion_development1;
pub mod q11010_new_potion_development2;
pub mod q11011_new_potion_development3;
pub mod q11012_future_elves;
pub mod q11013_shilens_hunt;
pub mod q11014_surprise_gift;
pub mod q11015_prepare_for_trade1;
pub mod q11016_prepare_for_trade2;
pub mod q11017_prepare_for_trade3;
pub mod q11018_future_dark_elves;
pub mod q11019_tribal_benefit;
pub mod q11020_blacksmiths_request;
pub mod q11021_red_gem_necklace1;
pub mod q11022_red_gem_necklace2;
pub mod q11023_red_gem_necklace3;
pub mod quest_common;
pub mod sailren_altar;
pub mod shadow_weapons;
pub(crate) mod sin_eater;
mod teleport_to_race_track;
pub mod teleport_with_charm;
pub mod third_class_transfer_quest;
mod toma;
mod tribe_support;
mod tunatun;
pub mod tutorial;
pub mod tvt;
mod valakas_teleporters;
pub mod wyvern_manager;

use std::sync::Arc;

use crate::game_loop::quests::{QuestRegistry, QuestScript};

/// Java's `ScriptEngineManager.executeScriptList()` + the `Quest`
/// constructor self-registration, collapsed into one boot-time list.
pub fn build_registry() -> QuestRegistry {
    let scripts: Vec<Arc<dyn QuestScript>> = vec![
        Arc::new(q00032_an_obvious_lie::Q00032AnObviousLie),
        Arc::new(q00033_make_a_pair_of_dress_shoes::Q00033MakeAPairOfDressShoes),
        Arc::new(q00034_in_search_of_cloth::Q00034InSearchOfCloth),
        Arc::new(q00035_find_glittering_jewelry::Q00035FindGlitteringJewelry),
        Arc::new(q00036_make_a_sewing_kit::Q00036MakeASewingKit),
        Arc::new(q00037_make_formal_wear::Q00037MakeFormalWear),
        Arc::new(q00042_help_the_uncle::Q00042HelpTheUncle),
        Arc::new(q00043_help_the_sister::Q00043HelpTheSister),
        Arc::new(q00044_help_the_son::Q00044HelpTheSon),
        Arc::new(q00109_in_search_of_the_nest::Q00109InSearchOfTheNest),
        Arc::new(q00210_obtain_a_wolf_pet::Q00210ObtainAWolfPet),
        Arc::new(q00257_the_guard_is_busy::Q00257TheGuardIsBusy),
        Arc::new(q00258_bring_wolf_pelts::Q00258BringWolfPelts),
        Arc::new(q00259_request_from_the_farm_owner::Q00259RequestFromTheFarmOwner),
        Arc::new(orc_amulet_hunt::OrcAmuletHunt::new(
            q00260_orc_hunting::data(),
        )),
        Arc::new(q00261_collectors_dream::Q00261CollectorsDream),
        Arc::new(q00262_trade_with_the_ivory_tower::Q00262TradeWithTheIvoryTower),
        Arc::new(q00264_keen_claws::Q00264KeenClaws),
        Arc::new(q00319_scent_of_death::Q00319ScentOfDeath),
        Arc::new(q00329_curiosity_of_a_dwarf::Q00329CuriosityOfADwarf),
        Arc::new(q00360_plunder_their_supplies::Q00360PlunderTheirSupplies),
        Arc::new(orc_amulet_hunt::OrcAmuletHunt::new(
            q00263_orc_subjugation::data(),
        )),
        Arc::new(q00265_bonds_of_slavery::Q00265BondsOfSlavery),
        Arc::new(q00266_pleas_of_pixies::Q00266PleasOfPixies),
        Arc::new(q00267_wrath_of_verdure::Q00267WrathOfVerdure),
        Arc::new(q00271_proof_of_valor::Q00271ProofOfValor),
        Arc::new(q00272_wrath_of_ancestors::Q00272WrathOfAncestors),
        Arc::new(q00274_skirmish_with_the_werewolves::Q00274SkirmishWithTheWerewolves),
        Arc::new(q00294_covert_business::Q00294CovertBusiness),
        Arc::new(q00297_gatekeepers_favor::Q00297GatekeepersFavor),
        Arc::new(q00326_vanquish_remnants::Q00326VanquishRemnants),
        Arc::new(q00328_sense_for_business::Q00328SenseForBusiness),
        Arc::new(q00331_arrow_of_vengeance::Q00331ArrowOfVengeance),
        Arc::new(q00273_invaders_of_the_holy_land::Q00273InvadersOfTheHolyLand),
        Arc::new(q00277_gatekeepers_offering::Q00277GatekeepersOffering),
        Arc::new(q00293_the_hidden_veins::Q00293TheHiddenVeins),
        Arc::new(q00295_dreaming_of_the_skies::Q00295DreamingOfTheSkies),
        Arc::new(q00296_tarantulas_spider_silk::Q00296TarantulasSpiderSilk),
        Arc::new(q00300_hunting_leto_lizardman::Q00300HuntingLetoLizardman),
        Arc::new(q00303_collect_arrowheads::Q00303CollectArrowheads),
        Arc::new(q00313_collect_spores::Q00313CollectSpores),
        Arc::new(q00316_destroy_plague_carriers::Q00316DestroyPlagueCarriers),
        Arc::new(q00317_catch_the_wind::Q00317CatchTheWind),
        Arc::new(q00320_bones_tell_the_future::Q00320BonesTellTheFuture),
        Arc::new(q00324_sweetest_venom::Q00324SweetestVenom),
        Arc::new(q00369_collector_of_jewels::Q00369CollectorOfJewels),
        Arc::new(q00619_relics_of_the_old_empire::Q00619RelicsOfTheOldEmpire),
        Arc::new(q00620_four_goblets::Q00620FourGoblets),
        Arc::new(q00623_the_finest_food::Q00623TheFinestFood),
        Arc::new(q00276_totem_of_the_hestui::Q00276TotemOfTheHestui),
        Arc::new(q00292_brigands_sweep::Q00292BrigandsSweep),
        Arc::new(q00617_gather_the_flames::Q00617GatherTheFlames),
        Arc::new(q00354_conquest_of_alligator_island::Q00354ConquestOfAlligatorIsland),
        Arc::new(q00356_dig_up_the_sea_of_spores::Q00356DigUpTheSeaOfSpores),
        Arc::new(q00358_illegitimate_child_of_the_goddess::Q00358IllegitimateChildOfTheGoddess),
        Arc::new(q00355_family_honor::Q00355FamilyHonor),
        Arc::new(q00622_specialty_liquor_delivery::Q00622SpecialtyLiquorDelivery),
        Arc::new(q00688_defeat_the_elrokian_raiders::Q00688DefeatTheElrokianRaiders),
        Arc::new(
            q10866_punitive_operation_on_the_devil_isle::Q10866PunitiveOperationOnTheDevilIsle,
        ),
        Arc::new(q11000_moon_knight::Q11000MoonKnight),
        Arc::new(q11001_tombs_of_ancestors::QUEST),
        Arc::new(q10993_future_dwarves::QUEST),
        Arc::new(q10994_future_orcs::QUEST),
        Arc::new(q11006_future_people::QUEST),
        Arc::new(q11012_future_elves::QUEST),
        Arc::new(q11018_future_dark_elves::QUEST),
        Arc::new(q10995_mutual_benefit::QUEST),
        Arc::new(q10996_temples_decorations::QUEST),
        Arc::new(q10997_loser_priest1::QUEST),
        Arc::new(q10998_loser_priest2::QUEST),
        Arc::new(q10999_loser_priest3::QUEST),
        Arc::new(q11002_help_with_temple_restoration::QUEST),
        Arc::new(q11003_perfect_leather_armor1::QUEST),
        Arc::new(q11004_perfect_leather_armor2::QUEST),
        Arc::new(q11005_perfect_leather_armor3::QUEST),
        Arc::new(q11007_noise_in_woods::QUEST),
        Arc::new(q11008_preparation_for_dungeon::QUEST),
        Arc::new(q11009_new_potion_development1::QUEST),
        Arc::new(q11010_new_potion_development2::QUEST),
        Arc::new(q11011_new_potion_development3::QUEST),
        Arc::new(q11013_shilens_hunt::QUEST),
        Arc::new(q11014_surprise_gift::QUEST),
        Arc::new(q11015_prepare_for_trade1::QUEST),
        Arc::new(q11016_prepare_for_trade2::QUEST),
        Arc::new(q11017_prepare_for_trade3::QUEST),
        Arc::new(q11019_tribal_benefit::QUEST),
        Arc::new(q11020_blacksmiths_request::QUEST),
        Arc::new(q11021_red_gem_necklace1::QUEST),
        Arc::new(q11022_red_gem_necklace2::QUEST),
        Arc::new(q11023_red_gem_necklace3::QUEST),
        Arc::new(q00641_attack_sailren::Q00641AttackSailren),
        Arc::new(q00642_a_powerful_primeval_creature::Q00642APowerfulPrimevalCreature),
        Arc::new(q00110_to_the_primeval_isle::Q00110ToThePrimevalIsle),
        Arc::new(q00374_whisper_of_dreams_part1::Q00374WhisperOfDreamsPart1),
        Arc::new(q00628_hunt_golden_ram::Q00628HuntGoldenRam),
        Arc::new(q00127_fishing_specialists_request::Q00127FishingSpecialistsRequest),
        Arc::new(q00306_crystal_of_fire_and_ice::Q00306CrystalOfFireAndIce),
        Arc::new(q00375_whisper_of_dreams_part2::Q00375WhisperOfDreamsPart2),
        Arc::new(q00606_battle_against_varka_silenos::Q00606BattleAgainstVarkaSilenos),
        Arc::new(q00612_battle_against_ketra_orcs::Q00612BattleAgainstKetraOrcs),
        Arc::new(q00634_in_search_of_fragments_of_dimension::Q00634InSearchOfFragmentsOfDimension),
        Arc::new(q00124_meeting_the_elroki::Q00124MeetingTheElroki),
        Arc::new(q00125_the_name_of_evil_1::Q00125TheNameOfEvil1),
        Arc::new(q00126_the_name_of_evil_2::Q00126TheNameOfEvil2),
        Arc::new(q00325_grim_collector::Q00325GrimCollector),
        Arc::new(
            q00643_rise_and_fall_of_the_elroki_tribe::Q00643RiseAndFallOfTheElrokiTribe::new(),
        ),
        Arc::new(q00111_elrokian_hunters_proof::Q00111ElrokianHuntersProof),
        Arc::new(q00373_supplier_of_reagents::Q00373SupplierOfReagents),
        Arc::new(q00344_1000_years_the_end_of_lamentation::Q003441000YearsTheEndOfLamentation),
        Arc::new(q00420_little_wing::Q00420LittleWing),
        Arc::new(q00421_little_wings_big_adventure::Q00421LittleWingsBigAdventure),
        Arc::new(alliance::AllianceQuest::new(
            q00605_alliance_with_ketra_orcs::data(),
        )),
        Arc::new(alliance::AllianceQuest::new(
            q00611_alliance_with_varka_silenos::data(),
        )),
        Arc::new(q00275_dark_winged_spies::Q00275DarkWingedSpies),
        Arc::new(q00370_an_elder_sows_seeds::Q00370AnElderSowsSeeds),
        Arc::new(q00640_the_zero_hour::Q00640TheZeroHour),
        Arc::new(q00327_recover_the_farmland::Q00327RecoverTheFarmland),
        Arc::new(q00348_an_arrogant_search::Q00348AnArrogantSearch),
        Arc::new(q00662_a_game_of_cards::Q00662AGameOfCards),
        Arc::new(q00333_hunt_of_the_black_lion::Q00333HuntOfTheBlackLion::new()),
        Arc::new(alliance_master::AllianceMaster),
        Arc::new(clan_master::ClanMaster),
        Arc::new(newbie_guide::NewbieGuide),
        Arc::new(tutorial::Tutorial),
        Arc::new(q10331_start_of_fate::Q10331StartOfFate),
        Arc::new(labyrinth_of_belis::LabyrinthOfBelis),
        Arc::new(q10360_certification_of_fate::Q10360CertificationOfFate),
        Arc::new(third_class_transfer_quest::HUMANS_FATE),
        Arc::new(third_class_transfer_quest::ELVEN_FATE),
        Arc::new(third_class_transfer_quest::DARK_ELFS_FATE),
        Arc::new(third_class_transfer_quest::ORCS_FATE),
        Arc::new(third_class_transfer_quest::DWARFS_FATE),
        Arc::new(third_class_transfer_quest::KAMAELS_FATE),
        Arc::new(evil_incubator::EvilIncubator),
        Arc::new(q10338_seize_your_destiny::Q10338SeizeYourDestiny),
        Arc::new(harnak_underground_ruins::HarnakUndergroundRuins),
        Arc::new(awakening_master::AwakeningMaster),
        // Salvation S8 — the Ertheia class transfers and their instances.
        Arc::new(q10751_winds_of_fate_encounters::Q10751WindsOfFateEncounters),
        Arc::new(mysterious_wizard::MysteriousWizard),
        Arc::new(q10752_winds_of_fate_a_promise::Q10752WindsOfFateAPromise),
        Arc::new(fortress_of_the_dead::FortressOfTheDead),
        Arc::new(q10753_winds_of_fate_choices::Q10753WindsOfFateChoices),
        Arc::new(chamber_of_prophecies::ChamberOfProphecies),
        Arc::new(wyvern_manager::WyvernManager),
        Arc::new(oly_buffer::OlyBuffer),
        Arc::new(oly_manager::OlyManager),
        Arc::new(monument_of_heroes::MonumentOfHeroes),
        Arc::new(nobless_master::NoblessMaster),
        Arc::new(shadow_weapons::ShadowWeapons),
        Arc::new(npc_location_info::NpcLocationInfo),
        Arc::new(teleport_to_race_track::TeleportToRaceTrack),
        Arc::new(antharas_heart::AntharasHeart),
        Arc::new(last_imperial_tomb::LastImperialTomb),
        Arc::new(sailren_altar::SailrenAltar),
        Arc::new(baium::Baium),
        Arc::new(castle_chamberlain::CastleChamberlain),
        Arc::new(clan_hall_auctioneer::ClanHallAuctioneer),
        Arc::new(clan_hall_door_manager::ClanHallDoorManager),
        Arc::new(clan_hall_manager::ClanHallManager),
        Arc::new(valakas_teleporters::ValakasTeleporters),
        Arc::new(dr_chaos_talk::DrChaosTalk),
        Arc::new(teleport_with_charm::TeleportWithCharm),
        Arc::new(tvt::Tvt),
        // ai/areas — slice 1: talk/teleporter NPCs.
        Arc::new(toma::Toma),
        Arc::new(elroki_teleporters::ElrokiTeleporters),
        Arc::new(pagan_teleporters::PaganTeleporters),
        Arc::new(tunatun::Tunatun),
        // ai/areas — slice 2: the small combat scripts.
        Arc::new(cave_maiden::CaveMaiden),
        Arc::new(pytan::Pytan),
        Arc::new(frozen_labyrinth::FrozenLabyrinth),
        Arc::new(pagan_keys::PaganKeys),
        Arc::new(hot_springs::HotSprings),
        Arc::new(plains_of_dion::PlainsOfDion),
        // ai/areas — slice 3: Den of Evil's Ragna Orcs (the zone's own
        // Kasha-eye script is @Disabled on this dist with no eye spawns).
        Arc::new(den_of_evil::RagnaOrcLeaders),
        Arc::new(den_of_evil::FrightenedRagnaOrc),
        // ai/areas — slice 4: the allied-tribe service NPCs (mirror pair).
        Arc::new(tribe_support::KetraOrcSupport),
        Arc::new(tribe_support::VarkaSilenosSupport),
        // ai/areas — slice 5.
        Arc::new(forge_of_the_gods::ForgeOfTheGods),
        // ai/areas — slice 6: the Beast Farm feeding chain (BeastFarm.java's
        // Gracia revamp NPCs never spawn on this dist — dead content).
        Arc::new(feedable_beasts::FeedableBeasts),
        // ai/areas — slice 7: Primeval Isle (MonasteryOfSilence.java is dead
        // content on this dist — none of its Gracia-era NPCs spawn).
        Arc::new(primeval_isle::PrimevalIsle),
        // ai/areas — slice 8: the Four Sepulchers party dungeon.
        Arc::new(four_sepulchers::FourSepulchers),
        Arc::new(four_sepulchers::FourSepulchersBosses),
        // ai/others — slice 1: the three wandering Mammon merchants (their
        // 30-minute relocation beat lives in `game_loop::area_npcs`).
        Arc::new(mammons::MerchantOfMammon),
        Arc::new(mammons::BlacksmithOfMammon),
        Arc::new(mammons::PriestOfMammon),
        // ai/others — slice 2: the castle staff (CastleSideEffect is skipped —
        // it pushes the later-chronicle `ExCastleState`).
        Arc::new(castle_services::CastleBlacksmith),
        Arc::new(castle_services::CastleWarehouse),
        Arc::new(castle_services::CastleMercenaryManager),
        Arc::new(castle_services::CastleDoorManager),
        Arc::new(castle_services::CastleSiegeManager),
        Arc::new(castle_services::CastleTeleporter),
        // ai/others — slice 3: the small combat behaviours.
        Arc::new(mob_behaviours::PolymorphingOnAttack),
        Arc::new(mob_behaviours::PolymorphingAngel),
        Arc::new(mob_behaviours::TimakOrcTroopLeader),
        Arc::new(mob_behaviours::FleeMonsters),
        Arc::new(mob_behaviours::FairyTrees),
        Arc::new(mob_behaviours::NonLethalableNpcs),
        // ai/others — slice 5: the talk/utility tail.
        Arc::new(misc_npcs::ArenaManager),
        Arc::new(misc_npcs::ToIVortex),
        Arc::new(misc_npcs::SymbolMaker),
        Arc::new(misc_npcs::RandomWalkingGuards),
    ];
    QuestRegistry::new(scripts)
}
