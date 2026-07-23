//! The compiled-in scripts — the Rust counterpart of
//! `dist/game/data/scripts/**.java` (which Java compiles at boot; here each
//! script is a module registering a `QuestScript` trait object in
//! [`build_registry`]). Framework in `game_loop/quests.rs`; this module is
//! only the content.

pub mod alliance_master;
pub mod clan_master;
pub mod newbie_guide;
pub mod npc_location_info;
pub mod quest_common;
pub mod dark_elf_change1;
pub mod dwarf_change1;
pub mod dwarf_change2;
pub mod first_class_transfer_talk;
pub mod elf_human_change1;
pub mod elf_human_change2;
pub mod orc_change1;
pub mod orc_dark_elf_change2;
pub mod q00109_in_search_of_the_nest;
pub mod q00210_obtain_a_wolf_pet;
pub mod q00257_the_guard_is_busy;
pub mod q00258_bring_wolf_pelts;
pub mod q00259_request_from_the_farm_owner;
pub mod q00260_orc_hunting;
pub mod q00261_collectors_dream;
pub mod q00262_trade_with_the_ivory_tower;
pub mod q00264_keen_claws;
pub mod q00319_scent_of_death;
pub mod q00329_curiosity_of_a_dwarf;
pub mod q00360_plunder_their_supplies;
pub mod q00263_orc_subjugation;
pub mod q00265_bonds_of_slavery;
pub mod q00266_pleas_of_pixies;
pub mod q00267_wrath_of_verdure;
pub mod q00271_proof_of_valor;
pub mod q00272_wrath_of_ancestors;
pub mod q00274_skirmish_with_the_werewolves;
pub mod q00294_covert_business;
pub mod q00297_gatekeepers_favor;
pub mod q00326_vanquish_remnants;
pub mod q00328_sense_for_business;
pub mod q00331_arrow_of_vengeance;
pub mod q00273_invaders_of_the_holy_land;
pub mod q00277_gatekeepers_offering;
pub mod q00293_the_hidden_veins;
pub mod q00295_dreaming_of_the_skies;
pub mod q00296_tarantulas_spider_silk;
pub mod q00300_hunting_leto_lizardman;
pub mod q00303_collect_arrowheads;
pub mod q00313_collect_spores;
pub mod q00316_destroy_plague_carriers;
pub mod q00317_catch_the_wind;
pub mod q00320_bones_tell_the_future;
pub mod q00401_path_of_the_warrior;
pub mod q00402_path_of_the_human_knight;
pub mod q00403_path_of_the_rogue;
pub mod q00404_path_of_the_human_wizard;
pub mod q00405_path_of_the_cleric;
pub mod q00406_path_of_the_elven_knight;
pub mod q00408_path_of_the_elven_wizard;
pub mod q00409_path_of_the_elven_oracle;
pub mod q00410_path_of_the_palus_knight;
pub mod q00411_path_of_the_assassin;
pub mod q00412_path_of_the_dark_wizard;
pub mod q00413_path_of_the_shillien_oracle;
pub mod q00414_path_of_the_orc_raider;
pub mod q00415_path_of_the_orc_monk;
pub mod q00416_path_of_the_orc_shaman;
pub mod q00417_path_of_the_scavenger;
pub mod q00418_path_of_the_artisan;
pub mod q00407_path_of_the_elven_scout;
pub mod q00324_sweetest_venom;
pub mod q00369_collector_of_jewels;
pub mod q00619_relics_of_the_old_empire;
pub mod q00623_the_finest_food;
pub mod q00276_totem_of_the_hestui;
pub mod q00292_brigands_sweep;
pub mod q00617_gather_the_flames;
pub mod q00354_conquest_of_alligator_island;
pub mod q00356_dig_up_the_sea_of_spores;
pub mod q00358_illegitimate_child_of_the_goddess;
pub mod q00355_family_honor;
pub mod q00622_specialty_liquor_delivery;
pub mod q00688_defeat_the_elrokian_raiders;
pub mod q00110_to_the_primeval_isle;
pub mod q00374_whisper_of_dreams_part1;
pub mod q00628_hunt_golden_ram;
pub mod q00127_fishing_specialists_request;
pub mod q00306_crystal_of_fire_and_ice;
pub mod q00375_whisper_of_dreams_part2;
pub mod q00606_battle_against_varka_silenos;
pub mod q00612_battle_against_ketra_orcs;
pub mod q00634_in_search_of_fragments_of_dimension;
pub mod q00124_meeting_the_elroki;
pub mod q00325_grim_collector;
pub mod q00643_rise_and_fall_of_the_elroki_tribe;
pub mod q00111_elrokian_hunters_proof;
pub mod q00373_supplier_of_reagents;
pub mod q00344_1000_years_the_end_of_lamentation;
pub mod q00235_mimirs_elixir;
pub mod q00222_test_of_the_duelist;
pub mod q00223_test_of_the_champion;
pub mod q00224_test_of_sagittarius;
pub mod q00211_trial_of_the_challenger;
pub mod q00225_test_of_the_searcher;
pub mod q00231_test_of_the_maestro;
pub mod antharas_heart;
mod valakas_teleporters;
mod dr_chaos_talk;
mod teleport_to_race_track;
pub mod teleport_with_charm;

use std::sync::Arc;

use crate::game_loop::quests::{QuestRegistry, QuestScript};

/// Java's `ScriptEngineManager.executeScriptList()` + the `Quest`
/// constructor self-registration, collapsed into one boot-time list.
pub fn build_registry() -> QuestRegistry {
    let scripts: Vec<Arc<dyn QuestScript>> = vec![
        Arc::new(q00109_in_search_of_the_nest::Q00109InSearchOfTheNest),
        Arc::new(q00210_obtain_a_wolf_pet::Q00210ObtainAWolfPet),
        Arc::new(q00257_the_guard_is_busy::Q00257TheGuardIsBusy),
        Arc::new(q00258_bring_wolf_pelts::Q00258BringWolfPelts),
        Arc::new(q00259_request_from_the_farm_owner::Q00259RequestFromTheFarmOwner),
        Arc::new(q00260_orc_hunting::Q00260OrcHunting),
        Arc::new(q00261_collectors_dream::Q00261CollectorsDream),
        Arc::new(q00262_trade_with_the_ivory_tower::Q00262TradeWithTheIvoryTower),
        Arc::new(q00264_keen_claws::Q00264KeenClaws),
        Arc::new(q00319_scent_of_death::Q00319ScentOfDeath),
        Arc::new(q00329_curiosity_of_a_dwarf::Q00329CuriosityOfADwarf),
        Arc::new(q00360_plunder_their_supplies::Q00360PlunderTheirSupplies),
        Arc::new(q00263_orc_subjugation::Q00263OrcSubjugation),
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
        Arc::new(q00325_grim_collector::Q00325GrimCollector),
        Arc::new(q00643_rise_and_fall_of_the_elroki_tribe::Q00643RiseAndFallOfTheElrokiTribe::new()),
        Arc::new(q00111_elrokian_hunters_proof::Q00111ElrokianHuntersProof),
        Arc::new(q00373_supplier_of_reagents::Q00373SupplierOfReagents),
        Arc::new(q00344_1000_years_the_end_of_lamentation::Q003441000YearsTheEndOfLamentation),
        Arc::new(q00235_mimirs_elixir::Q00235MimirsElixir),
        Arc::new(q00222_test_of_the_duelist::Q00222TestOfTheDuelist),
        Arc::new(q00223_test_of_the_champion::Q00223TestOfTheChampion),
        Arc::new(q00224_test_of_sagittarius::Q00224TestOfSagittarius),
        Arc::new(q00211_trial_of_the_challenger::Q00211TrialOfTheChallenger),
        Arc::new(q00225_test_of_the_searcher::Q00225TestOfTheSearcher),
        Arc::new(q00231_test_of_the_maestro::Q00231TestOfTheMaestro),
        Arc::new(q00401_path_of_the_warrior::Q00401PathOfTheWarrior),
        Arc::new(q00402_path_of_the_human_knight::Q00402PathOfTheHumanKnight),
        Arc::new(q00403_path_of_the_rogue::Q00403PathOfTheRogue),
        Arc::new(q00404_path_of_the_human_wizard::Q00404PathOfTheHumanWizard),
        Arc::new(q00405_path_of_the_cleric::Q00405PathOfTheCleric),
        Arc::new(q00406_path_of_the_elven_knight::Q00406PathOfTheElvenKnight),
        Arc::new(q00408_path_of_the_elven_wizard::Q00408PathOfTheElvenWizard),
        Arc::new(q00409_path_of_the_elven_oracle::Q00409PathOfTheElvenOracle),
        Arc::new(q00410_path_of_the_palus_knight::Q00410PathOfThePalusKnight),
        Arc::new(q00411_path_of_the_assassin::Q00411PathOfTheAssassin),
        Arc::new(q00412_path_of_the_dark_wizard::Q00412PathOfTheDarkWizard),
        Arc::new(q00413_path_of_the_shillien_oracle::Q00413PathOfTheShillienOracle),
        Arc::new(q00414_path_of_the_orc_raider::Q00414PathOfTheOrcRaider),
        Arc::new(q00415_path_of_the_orc_monk::Q00415PathOfTheOrcMonk),
        Arc::new(q00416_path_of_the_orc_shaman::Q00416PathOfTheOrcShaman),
        Arc::new(q00417_path_of_the_scavenger::Q00417PathOfTheScavenger),
        Arc::new(q00418_path_of_the_artisan::Q00418PathOfTheArtisan),
        Arc::new(q00407_path_of_the_elven_scout::Q00407PathOfTheElvenScout),
        Arc::new(alliance_master::AllianceMaster),
        Arc::new(clan_master::ClanMaster),
        Arc::new(newbie_guide::NewbieGuide),
        Arc::new(npc_location_info::NpcLocationInfo),
        Arc::new(dark_elf_change1::DarkElfChange1),
        Arc::new(first_class_transfer_talk::FirstClassTransferTalk),
        Arc::new(elf_human_change1::ElfHumanChange1::fighter()),
        Arc::new(elf_human_change1::ElfHumanChange1::wizard()),
        Arc::new(elf_human_change2::ElfHumanChange2::fighter()),
        Arc::new(elf_human_change2::ElfHumanChange2::wizard()),
        Arc::new(elf_human_change2::ElfHumanChange2::cleric()),
        Arc::new(dwarf_change2::DwarfChange2::blacksmith()),
        Arc::new(dwarf_change2::DwarfChange2::warehouse()),
        Arc::new(dwarf_change1::DwarfChange1::blacksmith()),
        Arc::new(dwarf_change1::DwarfChange1::warehouse()),
        Arc::new(orc_change1::OrcChange1),
        Arc::new(orc_dark_elf_change2::Change2::orc()),
        Arc::new(orc_dark_elf_change2::Change2::dark_elf()),
        Arc::new(teleport_to_race_track::TeleportToRaceTrack),
        Arc::new(antharas_heart::AntharasHeart),
        Arc::new(valakas_teleporters::ValakasTeleporters),
        Arc::new(dr_chaos_talk::DrChaosTalk),
        Arc::new(teleport_with_charm::TeleportWithCharm),
    ];
    QuestRegistry::new(scripts)
}
