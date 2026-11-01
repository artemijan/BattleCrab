/*
 * Copyright (c) 2013 L2jMobius
 * 
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 * 
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 * 
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
 * WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
 * IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
package handlers;

import org.l2jmobius.gameserver.handler.SkillConditionHandler;

import handlers.skill.conditions.*;

/**
 * @author NosBit, Mobius
 */
public class SkillConditionMasterHandler
{
	public static void main(String[] args)
	{
		SkillConditionHandler.getInstance().registerHandler("BuildAdvanceBase", BuildAdvanceBaseSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("BuildCamp", BuildCampSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanAddMaxEntranceInzone", CanAddMaxEntranceInzoneSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanBookmarkAddSlot", CanBookmarkAddSlotSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanChangeVitalItemCount", CanChangeVitalItemCountSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanEnchantAttribute", CanEnchantAttributeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanMountForEvent", CanMountForEventSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CannotUseInTransform", CannotUseInTransformSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanRefuelAirship", CanRefuelAirshipSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanSummon", CanSummonSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanSummonCubic", CanSummonCubicSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanSummonMulti", CanSummonMultiSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanSummonPet", CanSummonPetSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanSummonSiegeGolem", CanSummonSiegeGolemSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanTransform", CanTransformSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanTransformInDominion", CanTransformInDominionSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanUntransform", CanUntransformSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanUseInBattlefield", CanUseInBattlefieldSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanUseInDragonLair", CanUseInDragonLairSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanUseSwoopCannon", CanUseSwoopCannonSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CanUseVitalityIncreaseItem", CanUseVitalityIncreaseItemSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CheckLevel", CheckLevelSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("CheckSex", CheckSexSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("ConsumeBody", ConsumeBodySkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("EnergySaved", EnergySavedSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("EquipArmor", EquipArmorSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("EquippedCloakEnchant", EquippedCloakEnchantSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("EquipShield", EquipShieldSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("EquipSigil", EquipSigilSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("EquipWeapon", EquipWeaponSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("MaxMpSkillCondition", MaxMpSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("NotFeared", NotFearedSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("NotInUnderwater", NotInUnderwaterSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("Op1hWeapon", Op1hWeaponSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("Op2hWeapon", Op2hWeaponSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpAffectedBySkill", OpAffectedBySkillSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpAgathionEnergy", OpAgathionEnergySkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpAlignment", OpAlignmentSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpBaseStat", OpBaseStatSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpBlink", OpBlinkSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCallPc", OpCallPcSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCanEscape", OpCanEscapeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCanNotUseAirship", OpCanNotUseAirshipSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCannotUseTargetWithPrivateStore", OpCannotUseTargetWithPrivateStoreSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpChangeWeapon", OpChangeWeaponSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckAbnormal", OpCheckAbnormalSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckAccountType", OpCheckAccountTypeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckCastRange", OpCheckCastRangeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckClass", OpCheckClassSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckClassList", OpCheckClassListSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckCrtEffect", OpCheckCrtEffectSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckFlag", OpCheckFlagSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckOnGoingEventCampaign", OpCheckOnGoingEventCampaignSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckPcbangPoint", OpCheckPcbangPointSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckResidence", OpCheckResidenceSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckSkill", OpCheckSkillSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCheckSkillList", OpCheckSkillListSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpCompanion", OpCompanionSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpEnchantRange", OpEnchantRangeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpEncumbered", OpEncumberedSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpEnergyMax", OpEnergyMaxSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpEquipItem", OpEquipItemSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpExistNpc", OpExistNpcSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpFishingCast", OpFishingCastSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpFishingPumping", OpFishingPumpingSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpFishingReeling", OpFishingReelingSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpHaveSummon", OpHaveSummonSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpHaveSummonedNpc", OpHaveSummonedNpcSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpHome", OpHomeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpInSiege", OpInSiegeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpInstantzone", OpInstantzoneSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpInZone", OpInZoneSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpMagicWeapon", OpMagicWeaponSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpMainjob", OpMainjobSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNeedAgathion", OpNeedAgathionSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNeedSummonOrPet", OpNeedSummonOrPetSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNonMagicWeapon", OpNonMagicWeaponSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNotAffectedBySkill", OpNotAffectedBySkillSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNotCursed", OpNotCursedSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNotInEvent", OpNotInEventSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNotInPeacezone", OpNotInPeacezoneSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNotInstantzone", OpNotInstantzoneSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNotOlympiad", OpNotOlympiadSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpNotTerritory", OpNotTerritorySkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpOlympiad", OpOlympiadSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpPeacezone", OpPeacezoneSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpPkcount", OpPkcountSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpPledge", OpPledgeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpRestartPoint", OpRestartPointSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpResurrection", OpResurrectionSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpSiegeHammer", OpSiegeHammerSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpSkill", OpSkillSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpSkillAcquire", OpSkillAcquireSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpSocialClass", OpSocialClassSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpSoulMax", OpSoulMaxSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpStrider", OpStriderSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpSubjob", OpSubjobSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpSweeper", OpSweeperSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpTargetAllItemType", OpTargetAllItemTypeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpTargetArmorType", OpTargetArmorTypeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpTargetDoor", OpTargetDoorSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpTargetMyPledgeAcademy", OpTargetMyPledgeAcademySkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpTargetNpc", OpTargetNpcSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpTargetPc", OpTargetPcSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpTargetWeaponAttackType", OpTargetWeaponAttackTypeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpTerritory", OpTerritorySkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpUnlock", OpUnlockSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpUseFirecracker", OpUseFirecrackerSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpUsePraseed", OpUsePraseedSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("OpWyvern", OpWyvernSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("PossessHolything", PossessHolythingSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("RemainCpPer", RemainCpPerSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("RemainHpPer", RemainHpPerSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("RemainMpPer", RemainMpPerSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("SoulSaved", SoulSavedSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("TargetAffectedBySkill", TargetAffectedBySkillSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("TargetItemCrystalType", TargetItemCrystalTypeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("TargetMyMentee", TargetMyMenteeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("TargetMyParty", TargetMyPartySkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("TargetMyPledge", TargetMyPledgeSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("TargetNotAffectedBySkill", TargetNotAffectedBySkillSkillCondition::new);
		SkillConditionHandler.getInstance().registerHandler("TargetRace", TargetRaceSkillCondition::new);
	}
}
