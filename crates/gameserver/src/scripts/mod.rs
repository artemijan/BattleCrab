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
pub mod q00258_bring_wolf_pelts;
pub mod q00260_orc_hunting;
pub mod q00263_orc_subjugation;
pub mod q00265_bonds_of_slavery;
pub mod q00273_invaders_of_the_holy_land;
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
pub mod q00407_path_of_the_elven_scout;
pub mod q00324_sweetest_venom;
pub mod teleport_to_race_track;
pub mod teleport_with_charm;

use std::sync::Arc;

use crate::game_loop::quests::{QuestRegistry, QuestScript};

/// Java's `ScriptEngineManager.executeScriptList()` + the `Quest`
/// constructor self-registration, collapsed into one boot-time list.
pub fn build_registry() -> QuestRegistry {
    let scripts: Vec<Arc<dyn QuestScript>> = vec![
        Arc::new(q00109_in_search_of_the_nest::Q00109InSearchOfTheNest),
        Arc::new(q00258_bring_wolf_pelts::Q00258BringWolfPelts),
        Arc::new(q00260_orc_hunting::Q00260OrcHunting),
        Arc::new(q00263_orc_subjugation::Q00263OrcSubjugation),
        Arc::new(q00265_bonds_of_slavery::Q00265BondsOfSlavery),
        Arc::new(q00273_invaders_of_the_holy_land::Q00273InvadersOfTheHolyLand),
        Arc::new(q00303_collect_arrowheads::Q00303CollectArrowheads),
        Arc::new(q00313_collect_spores::Q00313CollectSpores),
        Arc::new(q00316_destroy_plague_carriers::Q00316DestroyPlagueCarriers),
        Arc::new(q00317_catch_the_wind::Q00317CatchTheWind),
        Arc::new(q00320_bones_tell_the_future::Q00320BonesTellTheFuture),
        Arc::new(q00324_sweetest_venom::Q00324SweetestVenom),
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
        Arc::new(teleport_with_charm::TeleportWithCharm),
    ];
    QuestRegistry::new(scripts)
}
