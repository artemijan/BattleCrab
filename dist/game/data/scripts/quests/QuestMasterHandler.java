/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package quests;

import java.util.logging.Level;
import java.util.logging.Logger;

import quests.Q00032_AnObviousLie.Q00032_AnObviousLie;
import quests.Q00033_MakeAPairOfDressShoes.Q00033_MakeAPairOfDressShoes;
import quests.Q00034_InSearchOfCloth.Q00034_InSearchOfCloth;
import quests.Q00035_FindGlitteringJewelry.Q00035_FindGlitteringJewelry;
import quests.Q00036_MakeASewingKit.Q00036_MakeASewingKit;
import quests.Q00037_MakeFormalWear.Q00037_MakeFormalWear;
import quests.Q00042_HelpTheUncle.Q00042_HelpTheUncle;
import quests.Q00043_HelpTheSister.Q00043_HelpTheSister;
import quests.Q00044_HelpTheSon.Q00044_HelpTheSon;
import quests.Q00070_SagaOfThePhoenixKnight.Q00070_SagaOfThePhoenixKnight;
import quests.Q00071_SagaOfEvasTemplar.Q00071_SagaOfEvasTemplar;
import quests.Q00072_SagaOfTheSwordMuse.Q00072_SagaOfTheSwordMuse;
import quests.Q00073_SagaOfTheDuelist.Q00073_SagaOfTheDuelist;
import quests.Q00074_SagaOfTheDreadnought.Q00074_SagaOfTheDreadnought;
import quests.Q00075_SagaOfTheTitan.Q00075_SagaOfTheTitan;
import quests.Q00076_SagaOfTheGrandKhavatari.Q00076_SagaOfTheGrandKhavatari;
import quests.Q00077_SagaOfTheDominator.Q00077_SagaOfTheDominator;
import quests.Q00078_SagaOfTheDoomcryer.Q00078_SagaOfTheDoomcryer;
import quests.Q00079_SagaOfTheAdventurer.Q00079_SagaOfTheAdventurer;
import quests.Q00080_SagaOfTheWindRider.Q00080_SagaOfTheWindRider;
import quests.Q00081_SagaOfTheGhostHunter.Q00081_SagaOfTheGhostHunter;
import quests.Q00082_SagaOfTheSagittarius.Q00082_SagaOfTheSagittarius;
import quests.Q00083_SagaOfTheMoonlightSentinel.Q00083_SagaOfTheMoonlightSentinel;
import quests.Q00084_SagaOfTheGhostSentinel.Q00084_SagaOfTheGhostSentinel;
import quests.Q00085_SagaOfTheCardinal.Q00085_SagaOfTheCardinal;
import quests.Q00086_SagaOfTheHierophant.Q00086_SagaOfTheHierophant;
import quests.Q00087_SagaOfEvasSaint.Q00087_SagaOfEvasSaint;
import quests.Q00088_SagaOfTheArchmage.Q00088_SagaOfTheArchmage;
import quests.Q00089_SagaOfTheMysticMuse.Q00089_SagaOfTheMysticMuse;
import quests.Q00090_SagaOfTheStormScreamer.Q00090_SagaOfTheStormScreamer;
import quests.Q00091_SagaOfTheArcanaLord.Q00091_SagaOfTheArcanaLord;
import quests.Q00092_SagaOfTheElementalMaster.Q00092_SagaOfTheElementalMaster;
import quests.Q00093_SagaOfTheSpectralMaster.Q00093_SagaOfTheSpectralMaster;
import quests.Q00094_SagaOfTheSoultaker.Q00094_SagaOfTheSoultaker;
import quests.Q00095_SagaOfTheHellKnight.Q00095_SagaOfTheHellKnight;
import quests.Q00096_SagaOfTheSpectralDancer.Q00096_SagaOfTheSpectralDancer;
import quests.Q00097_SagaOfTheShillienTemplar.Q00097_SagaOfTheShillienTemplar;
import quests.Q00098_SagaOfTheShillienSaint.Q00098_SagaOfTheShillienSaint;
import quests.Q00099_SagaOfTheFortuneSeeker.Q00099_SagaOfTheFortuneSeeker;
import quests.Q00100_SagaOfTheMaestro.Q00100_SagaOfTheMaestro;
import quests.Q00109_InSearchOfTheNest.Q00109_InSearchOfTheNest;
import quests.Q00110_ToThePrimevalIsle.Q00110_ToThePrimevalIsle;
import quests.Q00111_ElrokianHuntersProof.Q00111_ElrokianHuntersProof;
import quests.Q00124_MeetingTheElroki.Q00124_MeetingTheElroki;
import quests.Q00125_TheNameOfEvil1.Q00125_TheNameOfEvil1;
import quests.Q00126_TheNameOfEvil2.Q00126_TheNameOfEvil2;
import quests.Q00127_FishingSpecialistsRequest.Q00127_FishingSpecialistsRequest;
import quests.Q00210_ObtainAWolfPet.Q00210_ObtainAWolfPet;
import quests.Q00211_TrialOfTheChallenger.Q00211_TrialOfTheChallenger;
import quests.Q00212_TrialOfDuty.Q00212_TrialOfDuty;
import quests.Q00213_TrialOfTheSeeker.Q00213_TrialOfTheSeeker;
import quests.Q00214_TrialOfTheScholar.Q00214_TrialOfTheScholar;
import quests.Q00215_TrialOfThePilgrim.Q00215_TrialOfThePilgrim;
import quests.Q00216_TrialOfTheGuildsman.Q00216_TrialOfTheGuildsman;
import quests.Q00217_TestimonyOfTrust.Q00217_TestimonyOfTrust;
import quests.Q00218_TestimonyOfLife.Q00218_TestimonyOfLife;
import quests.Q00219_TestimonyOfFate.Q00219_TestimonyOfFate;
import quests.Q00220_TestimonyOfGlory.Q00220_TestimonyOfGlory;
import quests.Q00221_TestimonyOfProsperity.Q00221_TestimonyOfProsperity;
import quests.Q00222_TestOfTheDuelist.Q00222_TestOfTheDuelist;
import quests.Q00223_TestOfTheChampion.Q00223_TestOfTheChampion;
import quests.Q00224_TestOfSagittarius.Q00224_TestOfSagittarius;
import quests.Q00225_TestOfTheSearcher.Q00225_TestOfTheSearcher;
import quests.Q00226_TestOfTheHealer.Q00226_TestOfTheHealer;
import quests.Q00227_TestOfTheReformer.Q00227_TestOfTheReformer;
import quests.Q00228_TestOfMagus.Q00228_TestOfMagus;
import quests.Q00229_TestOfWitchcraft.Q00229_TestOfWitchcraft;
import quests.Q00230_TestOfTheSummoner.Q00230_TestOfTheSummoner;
import quests.Q00231_TestOfTheMaestro.Q00231_TestOfTheMaestro;
import quests.Q00232_TestOfTheLord.Q00232_TestOfTheLord;
import quests.Q00233_TestOfTheWarSpirit.Q00233_TestOfTheWarSpirit;
import quests.Q00234_FatesWhisper.Q00234_FatesWhisper;
import quests.Q00235_MimirsElixir.Q00235_MimirsElixir;
import quests.Q00255_Tutorial.Q00255_Tutorial;
import quests.Q00257_TheGuardIsBusy.Q00257_TheGuardIsBusy;
import quests.Q00258_BringWolfPelts.Q00258_BringWolfPelts;
import quests.Q00259_RequestFromTheFarmOwner.Q00259_RequestFromTheFarmOwner;
import quests.Q00260_OrcHunting.Q00260_OrcHunting;
import quests.Q00261_CollectorsDream.Q00261_CollectorsDream;
import quests.Q00262_TradeWithTheIvoryTower.Q00262_TradeWithTheIvoryTower;
import quests.Q00263_OrcSubjugation.Q00263_OrcSubjugation;
import quests.Q00264_KeenClaws.Q00264_KeenClaws;
import quests.Q00265_BondsOfSlavery.Q00265_BondsOfSlavery;
import quests.Q00266_PleasOfPixies.Q00266_PleasOfPixies;
import quests.Q00267_WrathOfVerdure.Q00267_WrathOfVerdure;
import quests.Q00271_ProofOfValor.Q00271_ProofOfValor;
import quests.Q00272_WrathOfAncestors.Q00272_WrathOfAncestors;
import quests.Q00273_InvadersOfTheHolyLand.Q00273_InvadersOfTheHolyLand;
import quests.Q00274_SkirmishWithTheWerewolves.Q00274_SkirmishWithTheWerewolves;
import quests.Q00275_DarkWingedSpies.Q00275_DarkWingedSpies;
import quests.Q00276_TotemOfTheHestui.Q00276_TotemOfTheHestui;
import quests.Q00277_GatekeepersOffering.Q00277_GatekeepersOffering;
import quests.Q00292_BrigandsSweep.Q00292_BrigandsSweep;
import quests.Q00293_TheHiddenVeins.Q00293_TheHiddenVeins;
import quests.Q00294_CovertBusiness.Q00294_CovertBusiness;
import quests.Q00295_DreamingOfTheSkies.Q00295_DreamingOfTheSkies;
import quests.Q00296_TarantulasSpiderSilk.Q00296_TarantulasSpiderSilk;
import quests.Q00297_GatekeepersFavor.Q00297_GatekeepersFavor;
import quests.Q00300_HuntingLetoLizardman.Q00300_HuntingLetoLizardman;
import quests.Q00303_CollectArrowheads.Q00303_CollectArrowheads;
import quests.Q00306_CrystalOfFireAndIce.Q00306_CrystalOfFireAndIce;
import quests.Q00313_CollectSpores.Q00313_CollectSpores;
import quests.Q00316_DestroyPlagueCarriers.Q00316_DestroyPlagueCarriers;
import quests.Q00317_CatchTheWind.Q00317_CatchTheWind;
import quests.Q00319_ScentOfDeath.Q00319_ScentOfDeath;
import quests.Q00320_BonesTellTheFuture.Q00320_BonesTellTheFuture;
import quests.Q00324_SweetestVenom.Q00324_SweetestVenom;
import quests.Q00325_GrimCollector.Q00325_GrimCollector;
import quests.Q00326_VanquishRemnants.Q00326_VanquishRemnants;
import quests.Q00327_RecoverTheFarmland.Q00327_RecoverTheFarmland;
import quests.Q00328_SenseForBusiness.Q00328_SenseForBusiness;
import quests.Q00329_CuriosityOfADwarf.Q00329_CuriosityOfADwarf;
import quests.Q00331_ArrowOfVengeance.Q00331_ArrowOfVengeance;
import quests.Q00333_HuntOfTheBlackLion.Q00333_HuntOfTheBlackLion;
import quests.Q00344_1000YearsTheEndOfLamentation.Q00344_1000YearsTheEndOfLamentation;
import quests.Q00348_AnArrogantSearch.Q00348_AnArrogantSearch;
import quests.Q00350_EnhanceYourWeapon.Q00350_EnhanceYourWeapon;
import quests.Q00354_ConquestOfAlligatorIsland.Q00354_ConquestOfAlligatorIsland;
import quests.Q00355_FamilyHonor.Q00355_FamilyHonor;
import quests.Q00356_DigUpTheSeaOfSpores.Q00356_DigUpTheSeaOfSpores;
import quests.Q00358_IllegitimateChildOfTheGoddess.Q00358_IllegitimateChildOfTheGoddess;
import quests.Q00360_PlunderTheirSupplies.Q00360_PlunderTheirSupplies;
import quests.Q00369_CollectorOfJewels.Q00369_CollectorOfJewels;
import quests.Q00370_AnElderSowsSeeds.Q00370_AnElderSowsSeeds;
import quests.Q00373_SupplierOfReagents.Q00373_SupplierOfReagents;
import quests.Q00374_WhisperOfDreamsPart1.Q00374_WhisperOfDreamsPart1;
import quests.Q00375_WhisperOfDreamsPart2.Q00375_WhisperOfDreamsPart2;
import quests.Q00401_PathOfTheWarrior.Q00401_PathOfTheWarrior;
import quests.Q00402_PathOfTheHumanKnight.Q00402_PathOfTheHumanKnight;
import quests.Q00403_PathOfTheRogue.Q00403_PathOfTheRogue;
import quests.Q00404_PathOfTheHumanWizard.Q00404_PathOfTheHumanWizard;
import quests.Q00405_PathOfTheCleric.Q00405_PathOfTheCleric;
import quests.Q00406_PathOfTheElvenKnight.Q00406_PathOfTheElvenKnight;
import quests.Q00407_PathOfTheElvenScout.Q00407_PathOfTheElvenScout;
import quests.Q00408_PathOfTheElvenWizard.Q00408_PathOfTheElvenWizard;
import quests.Q00409_PathOfTheElvenOracle.Q00409_PathOfTheElvenOracle;
import quests.Q00410_PathOfThePalusKnight.Q00410_PathOfThePalusKnight;
import quests.Q00411_PathOfTheAssassin.Q00411_PathOfTheAssassin;
import quests.Q00412_PathOfTheDarkWizard.Q00412_PathOfTheDarkWizard;
import quests.Q00413_PathOfTheShillienOracle.Q00413_PathOfTheShillienOracle;
import quests.Q00414_PathOfTheOrcRaider.Q00414_PathOfTheOrcRaider;
import quests.Q00415_PathOfTheOrcMonk.Q00415_PathOfTheOrcMonk;
import quests.Q00416_PathOfTheOrcShaman.Q00416_PathOfTheOrcShaman;
import quests.Q00417_PathOfTheScavenger.Q00417_PathOfTheScavenger;
import quests.Q00418_PathOfTheArtisan.Q00418_PathOfTheArtisan;
import quests.Q00420_LittleWing.Q00420_LittleWing;
import quests.Q00421_LittleWingsBigAdventure.Q00421_LittleWingsBigAdventure;
import quests.Q00500_BrothersBoundInChains.Q00500_BrothersBoundInChains;
import quests.Q00605_AllianceWithKetraOrcs.Q00605_AllianceWithKetraOrcs;
import quests.Q00606_BattleAgainstVarkaSilenos.Q00606_BattleAgainstVarkaSilenos;
import quests.Q00611_AllianceWithVarkaSilenos.Q00611_AllianceWithVarkaSilenos;
import quests.Q00612_BattleAgainstKetraOrcs.Q00612_BattleAgainstKetraOrcs;
import quests.Q00617_GatherTheFlames.Q00617_GatherTheFlames;
import quests.Q00620_FourGoblets.Q00620_FourGoblets;
import quests.Q00619_RelicsOfTheOldEmpire.Q00619_RelicsOfTheOldEmpire;
import quests.Q00622_SpecialtyLiquorDelivery.Q00622_SpecialtyLiquorDelivery;
import quests.Q00623_TheFinestFood.Q00623_TheFinestFood;
import quests.Q00628_HuntGoldenRam.Q00628_HuntGoldenRam;
import quests.Q00634_InSearchOfFragmentsOfDimension.Q00634_InSearchOfFragmentsOfDimension;
import quests.Q00640_TheZeroHour.Q00640_TheZeroHour;
import quests.Q00641_AttackSailren.Q00641_AttackSailren;
import quests.Q00642_APowerfulPrimevalCreature.Q00642_APowerfulPrimevalCreature;
import quests.Q00643_RiseAndFallOfTheElrokiTribe.Q00643_RiseAndFallOfTheElrokiTribe;
import quests.Q00662_AGameOfCards.Q00662_AGameOfCards;
import quests.Q00688_DefeatTheElrokianRaiders.Q00688_DefeatTheElrokianRaiders;
import quests.Q00933_ExploringTheWestWingOfTheDungeonOfAbyss.Q00933_ExploringTheWestWingOfTheDungeonOfAbyss;
import quests.Q00935_ExploringTheEastWingOfTheDungeonOfAbyss.Q00935_ExploringTheEastWingOfTheDungeonOfAbyss;
import quests.Q10866_PunitiveOperationOnTheDevilIsle.Q10866_PunitiveOperationOnTheDevilIsle;
import quests.Q10993_FutureDwarves.Q10993_FutureDwarves;
import quests.Q10994_FutureOrcs.Q10994_FutureOrcs;
import quests.Q10995_MutualBenefit.Q10995_MutualBenefit;
import quests.Q10996_TemplesDecorations.Q10996_TemplesDecorations;
import quests.Q10997_LoserPriest1.Q10997_LoserPriest1;
import quests.Q10998_LoserPriest2.Q10998_LoserPriest2;
import quests.Q10999_LoserPriest3.Q10999_LoserPriest3;
import quests.Q11000_MoonKnight.Q11000_MoonKnight;
import quests.Q11001_TombsOfAncestors.Q11001_TombsOfAncestors;
import quests.Q11002_HelpWithTempleRestoration.Q11002_HelpWithTempleRestoration;
import quests.Q11003_PerfectLeatherArmor1.Q11003_PerfectLeatherArmor1;
import quests.Q11004_PerfectLeatherArmor2.Q11004_PerfectLeatherArmor2;
import quests.Q11005_PerfectLeatherArmor3.Q11005_PerfectLeatherArmor3;
import quests.Q11006_FuturePeople.Q11006_FuturePeople;
import quests.Q11007_NoiseInWoods.Q11007_NoiseInWoods;
import quests.Q11008_PreparationForDungeon.Q11008_PreparationForDungeon;
import quests.Q11009_NewPotionDevelopment1.Q11009_NewPotionDevelopment1;
import quests.Q11010_NewPotionDevelopment2.Q11010_NewPotionDevelopment2;
import quests.Q11011_NewPotionDevelopment3.Q11011_NewPotionDevelopment3;
import quests.Q11012_FutureElves.Q11012_FutureElves;
import quests.Q11013_ShilensHunt.Q11013_ShilensHunt;
import quests.Q11014_SurpriseGift.Q11014_SurpriseGift;
import quests.Q11015_PrepareForTrade1.Q11015_PrepareForTrade1;
import quests.Q11016_PrepareForTrade2.Q11016_PrepareForTrade2;
import quests.Q11017_PrepareForTrade3.Q11017_PrepareForTrade3;
import quests.Q11018_FutureDarkElves.Q11018_FutureDarkElves;
import quests.Q11019_TribalBenefit.Q11019_TribalBenefit;
import quests.Q11020_BlacksmithsRequest.Q11020_BlacksmithsRequest;
import quests.Q11021_RedGemNecklace1.Q11021_RedGemNecklace1;
import quests.Q11022_RedGemNecklace2.Q11022_RedGemNecklace2;
import quests.Q11023_RedGemNecklace3.Q11023_RedGemNecklace3;
import quests.not_done.Q00630_PirateTreasureHunt;
import quests.not_done.Q00657_MonsterArenaTrialForWarriors;
import quests.not_done.Q00658_MonsterArenaTrialForEliteWarriors;
import quests.not_done.Q00664_QuarrelsTime;
import quests.not_done.Q00910_RequestFromTheRedLibraGuildBasic;
import quests.not_done.Q00911_RequestFromTheRedLibraGuildIntermediate;
import quests.not_done.Q00912_RequestFromTheRedLibraGuildAdvanced;
import quests.not_done.Q00913_RequestFromTheRedLibraGuildSuperAdvanced;
import quests.not_done.Q10861_MonsterArenaTheBirthOfAWarrior;
import quests.not_done.Q10862_MonsterArenaChallenge10Battles;
import quests.not_done.Q10863_MonsterArenaNewChallenge15Battles;
import quests.not_done.Q10864_MonsterArenaBraveWarrior25Battles;
import quests.not_done.Q10865_MonsterArenaLastCall40Battles;
import quests.not_done.Q10867_GoneMissing;
import quests.not_done.Q10868_TheDarkSideOfPower;
import quests.not_done.Q10870_UnfinishedDevice;
import quests.not_done.Q10871_DeathToThePirateKing;

/**
 * @author NosBit
 */
public class QuestMasterHandler
{
	private static final Logger LOGGER = Logger.getLogger(QuestMasterHandler.class.getName());
	
	private static final Class<?>[] QUESTS =
	{
		Q00032_AnObviousLie.class, // FIXME: Grand Crusade
		Q00033_MakeAPairOfDressShoes.class, // FIXME: Grand Crusade
		Q00034_InSearchOfCloth.class, // FIXME: Grand Crusade
		Q00035_FindGlitteringJewelry.class, // FIXME: Grand Crusade
		Q00036_MakeASewingKit.class, // FIXME: Grand Crusade
		Q00037_MakeFormalWear.class, // FIXME: Grand Crusade
		Q00042_HelpTheUncle.class, // FIXME: Grand Crusade
		Q00043_HelpTheSister.class, // FIXME: Grand Crusade
		Q00044_HelpTheSon.class, // FIXME: Grand Crusade
		Q00070_SagaOfThePhoenixKnight.class,
		Q00071_SagaOfEvasTemplar.class,
		Q00072_SagaOfTheSwordMuse.class,
		Q00073_SagaOfTheDuelist.class,
		Q00074_SagaOfTheDreadnought.class,
		Q00075_SagaOfTheTitan.class,
		Q00076_SagaOfTheGrandKhavatari.class,
		Q00077_SagaOfTheDominator.class,
		Q00078_SagaOfTheDoomcryer.class,
		Q00079_SagaOfTheAdventurer.class,
		Q00080_SagaOfTheWindRider.class,
		Q00081_SagaOfTheGhostHunter.class,
		Q00082_SagaOfTheSagittarius.class,
		Q00083_SagaOfTheMoonlightSentinel.class,
		Q00084_SagaOfTheGhostSentinel.class,
		Q00085_SagaOfTheCardinal.class,
		Q00086_SagaOfTheHierophant.class,
		Q00087_SagaOfEvasSaint.class,
		Q00088_SagaOfTheArchmage.class,
		Q00089_SagaOfTheMysticMuse.class,
		Q00090_SagaOfTheStormScreamer.class,
		Q00091_SagaOfTheArcanaLord.class,
		Q00092_SagaOfTheElementalMaster.class,
		Q00093_SagaOfTheSpectralMaster.class,
		Q00094_SagaOfTheSoultaker.class,
		Q00095_SagaOfTheHellKnight.class,
		Q00096_SagaOfTheSpectralDancer.class,
		Q00097_SagaOfTheShillienTemplar.class,
		Q00098_SagaOfTheShillienSaint.class,
		Q00099_SagaOfTheFortuneSeeker.class,
		Q00100_SagaOfTheMaestro.class,
		Q00109_InSearchOfTheNest.class,
		Q00110_ToThePrimevalIsle.class, // FIXME: Grand Crusade
		Q00111_ElrokianHuntersProof.class, // FIXME: Grand Crusade
		Q00124_MeetingTheElroki.class, // FIXME: Grand Crusade
		Q00125_TheNameOfEvil1.class, // FIXME: Grand Crusade
		Q00126_TheNameOfEvil2.class, // FIXME: Grand Crusade
		Q00127_FishingSpecialistsRequest.class,
		Q00210_ObtainAWolfPet.class, // FIXME: Grand Crusade
		Q00211_TrialOfTheChallenger.class,
		Q00212_TrialOfDuty.class,
		Q00213_TrialOfTheSeeker.class,
		Q00214_TrialOfTheScholar.class,
		Q00215_TrialOfThePilgrim.class,
		Q00216_TrialOfTheGuildsman.class,
		Q00217_TestimonyOfTrust.class,
		Q00218_TestimonyOfLife.class,
		Q00219_TestimonyOfFate.class,
		Q00220_TestimonyOfGlory.class,
		Q00221_TestimonyOfProsperity.class,
		Q00222_TestOfTheDuelist.class,
		Q00223_TestOfTheChampion.class,
		Q00224_TestOfSagittarius.class,
		Q00225_TestOfTheSearcher.class,
		Q00226_TestOfTheHealer.class,
		Q00227_TestOfTheReformer.class,
		Q00228_TestOfMagus.class,
		Q00229_TestOfWitchcraft.class,
		Q00230_TestOfTheSummoner.class,
		Q00231_TestOfTheMaestro.class,
		Q00232_TestOfTheLord.class,
		Q00233_TestOfTheWarSpirit.class,
		Q00234_FatesWhisper.class,
		Q00235_MimirsElixir.class,
		Q00255_Tutorial.class,
		Q00257_TheGuardIsBusy.class,
		Q00258_BringWolfPelts.class,
		Q00259_RequestFromTheFarmOwner.class,
		Q00260_OrcHunting.class,
		Q00261_CollectorsDream.class,
		Q00262_TradeWithTheIvoryTower.class,
		Q00263_OrcSubjugation.class,
		Q00264_KeenClaws.class,
		Q00265_BondsOfSlavery.class,
		Q00266_PleasOfPixies.class,
		Q00267_WrathOfVerdure.class,
		Q00271_ProofOfValor.class,
		Q00272_WrathOfAncestors.class,
		Q00273_InvadersOfTheHolyLand.class,
		Q00274_SkirmishWithTheWerewolves.class,
		Q00275_DarkWingedSpies.class,
		Q00276_TotemOfTheHestui.class,
		Q00277_GatekeepersOffering.class,
		Q00292_BrigandsSweep.class,
		Q00293_TheHiddenVeins.class,
		Q00294_CovertBusiness.class,
		Q00295_DreamingOfTheSkies.class,
		Q00296_TarantulasSpiderSilk.class,
		Q00297_GatekeepersFavor.class,
		Q00300_HuntingLetoLizardman.class,
		Q00303_CollectArrowheads.class,
		Q00306_CrystalOfFireAndIce.class,
		Q00313_CollectSpores.class,
		Q00316_DestroyPlagueCarriers.class,
		Q00317_CatchTheWind.class,
		Q00319_ScentOfDeath.class,
		Q00320_BonesTellTheFuture.class,
		Q00324_SweetestVenom.class,
		Q00325_GrimCollector.class,
		Q00326_VanquishRemnants.class,
		Q00327_RecoverTheFarmland.class,
		Q00328_SenseForBusiness.class,
		Q00329_CuriosityOfADwarf.class,
		Q00331_ArrowOfVengeance.class,
		Q00333_HuntOfTheBlackLion.class,
		Q00344_1000YearsTheEndOfLamentation.class,
		Q00348_AnArrogantSearch.class,
		Q00350_EnhanceYourWeapon.class,
		Q00354_ConquestOfAlligatorIsland.class,
		Q00355_FamilyHonor.class,
		Q00356_DigUpTheSeaOfSpores.class,
		Q00358_IllegitimateChildOfTheGoddess.class,
		Q00360_PlunderTheirSupplies.class,
		Q00369_CollectorOfJewels.class,
		Q00370_AnElderSowsSeeds.class,
		Q00373_SupplierOfReagents.class,
		Q00374_WhisperOfDreamsPart1.class,
		Q00375_WhisperOfDreamsPart2.class,
		Q00401_PathOfTheWarrior.class,
		Q00402_PathOfTheHumanKnight.class,
		Q00403_PathOfTheRogue.class,
		Q00404_PathOfTheHumanWizard.class,
		Q00405_PathOfTheCleric.class,
		Q00406_PathOfTheElvenKnight.class,
		Q00407_PathOfTheElvenScout.class,
		Q00408_PathOfTheElvenWizard.class,
		Q00409_PathOfTheElvenOracle.class,
		Q00410_PathOfThePalusKnight.class,
		Q00411_PathOfTheAssassin.class,
		Q00412_PathOfTheDarkWizard.class,
		Q00413_PathOfTheShillienOracle.class,
		Q00414_PathOfTheOrcRaider.class,
		Q00415_PathOfTheOrcMonk.class,
		Q00416_PathOfTheOrcShaman.class,
		Q00417_PathOfTheScavenger.class,
		Q00418_PathOfTheArtisan.class,
		Q00420_LittleWing.class, // FIXME: Underground
		Q00421_LittleWingsBigAdventure.class, // FIXME: Underground
		Q00500_BrothersBoundInChains.class,
		Q00605_AllianceWithKetraOrcs.class,
		Q00606_BattleAgainstVarkaSilenos.class,
		Q00611_AllianceWithVarkaSilenos.class,
		Q00612_BattleAgainstKetraOrcs.class,
		Q00617_GatherTheFlames.class,
    Q00619_RelicsOfTheOldEmpire.class,
		Q00620_FourGoblets.class, // FIXME: Grand Crusade
		Q00622_SpecialtyLiquorDelivery.class,
		Q00623_TheFinestFood.class,
		Q00628_HuntGoldenRam.class,
		Q00630_PirateTreasureHunt.class, // TODO: Not done.
    Q00634_InSearchOfFragmentsOfDimension.class,
    Q00640_TheZeroHour.class,
		Q00641_AttackSailren.class, // FIXME: Grand Crusade
		Q00642_APowerfulPrimevalCreature.class, // FIXME: Grand Crusade
		Q00643_RiseAndFallOfTheElrokiTribe.class, // FIXME: Grand Crusade
		Q00657_MonsterArenaTrialForWarriors.class, // TODO: Not done.
		Q00658_MonsterArenaTrialForEliteWarriors.class, // TODO: Not done.
		Q00662_AGameOfCards.class,
		Q00664_QuarrelsTime.class, // TODO: Not done.
		Q00688_DefeatTheElrokianRaiders.class, // FIXME: Grand Crusade
		Q00910_RequestFromTheRedLibraGuildBasic.class, // TODO: Not done.
		Q00911_RequestFromTheRedLibraGuildIntermediate.class, // TODO: Not done.
		Q00912_RequestFromTheRedLibraGuildAdvanced.class, // TODO: Not done.
		Q00913_RequestFromTheRedLibraGuildSuperAdvanced.class, // TODO: Not done.
		Q00933_ExploringTheWestWingOfTheDungeonOfAbyss.class,
		Q00935_ExploringTheEastWingOfTheDungeonOfAbyss.class,
		// Q10506_DianasRequest.class, // TODO: Not done. NPC 9001 not in client.
		Q10861_MonsterArenaTheBirthOfAWarrior.class, // TODO: Not done.
		Q10862_MonsterArenaChallenge10Battles.class, // TODO: Not done.
		Q10863_MonsterArenaNewChallenge15Battles.class, // TODO: Not done.
		Q10864_MonsterArenaBraveWarrior25Battles.class, // TODO: Not done.
		Q10865_MonsterArenaLastCall40Battles.class, // TODO: Not done.
		Q10866_PunitiveOperationOnTheDevilIsle.class,
		Q10867_GoneMissing.class, // TODO: Not done.
		Q10868_TheDarkSideOfPower.class, // TODO: Not done.
		Q10870_UnfinishedDevice.class, // TODO: Not done.
		Q10871_DeathToThePirateKing.class, // TODO: Not done.
		Q10993_FutureDwarves.class,
		Q10994_FutureOrcs.class,
		Q10995_MutualBenefit.class,
		Q10996_TemplesDecorations.class,
		Q10997_LoserPriest1.class,
		Q10998_LoserPriest2.class,
		Q10999_LoserPriest3.class,
		Q11000_MoonKnight.class,
		Q11001_TombsOfAncestors.class,
		Q11002_HelpWithTempleRestoration.class,
		Q11003_PerfectLeatherArmor1.class,
		Q11004_PerfectLeatherArmor2.class,
		Q11005_PerfectLeatherArmor3.class,
		Q11006_FuturePeople.class,
		Q11007_NoiseInWoods.class,
		Q11008_PreparationForDungeon.class,
		Q11009_NewPotionDevelopment1.class,
		Q11010_NewPotionDevelopment2.class,
		Q11011_NewPotionDevelopment3.class,
		Q11012_FutureElves.class,
		Q11013_ShilensHunt.class,
		Q11014_SurpriseGift.class,
		Q11015_PrepareForTrade1.class,
		Q11016_PrepareForTrade2.class,
		Q11017_PrepareForTrade3.class,
		Q11018_FutureDarkElves.class,
		Q11019_TribalBenefit.class,
		Q11020_BlacksmithsRequest.class,
		Q11021_RedGemNecklace1.class,
		Q11022_RedGemNecklace2.class,
		Q11023_RedGemNecklace3.class,
	};
	
	public static void main(String[] args)
	{
		for (Class<?> quest : QUESTS)
		{
			try
			{
				quest.getDeclaredConstructor().newInstance();
			}
			catch (Exception e)
			{
				LOGGER.log(Level.SEVERE, QuestMasterHandler.class.getSimpleName() + ": Failed loading " + quest.getSimpleName() + ":", e);
			}
		}
	}
}
