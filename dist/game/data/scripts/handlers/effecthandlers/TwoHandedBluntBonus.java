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
package handlers.effecthandlers;

import org.l2jmobius.gameserver.enums.StatModifierType;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.conditions.Condition;
import org.l2jmobius.gameserver.model.conditions.ConditionUsingItemType;
import org.l2jmobius.gameserver.model.conditions.ConditionUsingSlotType;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.ItemTemplate;
import org.l2jmobius.gameserver.model.item.type.WeaponType;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * @author Sdw, Mobius
 */
public class TwoHandedBluntBonus extends AbstractEffect
{
	private static final Condition _weaponTypeCondition = new ConditionUsingItemType(WeaponType.BLUNT.mask());
	private static final Condition _slotCondition = new ConditionUsingSlotType(ItemTemplate.SLOT_LR_HAND);
	
	private final double _pAtkAmount;
	private final StatModifierType _pAtkMode;
	
	private final double _mAtkAmount;
	private final StatModifierType _mAtkMode;
	
	private final double _pAtkSpeedAmount;
	private final StatModifierType _pAtkSpeedMode;
	
	private final double _mAtkSpeedAmount;
	private final StatModifierType _mAtkSpeedMode;
	
	private final double _pAccuracyAmount;
	private final StatModifierType _pAccuracyMode;
	
	private final double _mAccuracyAmount;
	private final StatModifierType _mAccuracyMode;
	
	private final double _pCritRateAmount;
	private final StatModifierType _pCritRateMode;
	
	private final double _mCritRateAmount;
	private final StatModifierType _mCritRateMode;
	
	private final double _pCritDamageAmount;
	private final StatModifierType _pCritDamageMode;
	
	private final double _mCritDamageAmount;
	private final StatModifierType _mCritDamageMode;
	
	private final double _speedAmount;
	private final StatModifierType _speedMode;
	
	public TwoHandedBluntBonus(StatSet params)
	{
		_pAtkAmount = params.getDouble("pAtkAmount", 0);
		_pAtkMode = params.getEnum("pAtkMode", StatModifierType.class, StatModifierType.DIFF);
		
		_mAtkAmount = params.getDouble("mAtkAmount", 0);
		_mAtkMode = params.getEnum("mAtkMode", StatModifierType.class, StatModifierType.DIFF);
		
		_pAtkSpeedAmount = params.getDouble("pAtkSpeedAmount", 0);
		_pAtkSpeedMode = params.getEnum("pAtkSpeedMode", StatModifierType.class, StatModifierType.DIFF);
		
		_mAtkSpeedAmount = params.getDouble("mAtkSpeedAmount", 0);
		_mAtkSpeedMode = params.getEnum("mAtkSpeedMode", StatModifierType.class, StatModifierType.DIFF);
		
		_pAccuracyAmount = params.getDouble("pAccuracyAmount", 0);
		_pAccuracyMode = params.getEnum("pAccuracyMode", StatModifierType.class, StatModifierType.DIFF);
		
		_mAccuracyAmount = params.getDouble("mAccuracyAmount", 0);
		_mAccuracyMode = params.getEnum("mAccuracyMode", StatModifierType.class, StatModifierType.DIFF);
		
		_pCritRateAmount = params.getDouble("pCritRateAmount", 0);
		_pCritRateMode = params.getEnum("pCritRateMode", StatModifierType.class, StatModifierType.DIFF);
		
		_mCritRateAmount = params.getDouble("mCritRateAmount", 0);
		_mCritRateMode = params.getEnum("mCritRateMode", StatModifierType.class, StatModifierType.DIFF);
		
		_pCritDamageAmount = params.getDouble("pCritDamageAmount", 0);
		_pCritDamageMode = params.getEnum("pCritDamageMode", StatModifierType.class, StatModifierType.DIFF);
		
		_mCritDamageAmount = params.getDouble("mCritDamageAmount", 0);
		_mCritDamageMode = params.getEnum("mCritDamageMode", StatModifierType.class, StatModifierType.DIFF);
		
		_speedAmount = params.getDouble("speedAmount", 0);
		_speedMode = params.getEnum("speedMode", StatModifierType.class, StatModifierType.DIFF);
	}
	
	@Override
	public void pump(Creature effected, Skill skill)
	{
		if (_weaponTypeCondition.test(effected, effected, skill) && _slotCondition.test(effected, effected, skill))
		{
			if (_pAtkAmount != 0)
			{
				if (_pAtkMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.PHYSICAL_ATTACK, _pAtkAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.PHYSICAL_ATTACK, (_pAtkAmount / 100) + 1);
				}
			}
			
			if (_mAtkAmount != 0)
			{
				if (_mAtkMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.MAGIC_ATTACK, _mAtkAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.MAGIC_ATTACK, (_mAtkAmount / 100) + 1);
				}
			}
			
			if (_pAtkSpeedAmount != 0)
			{
				if (_pAtkSpeedMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.PHYSICAL_ATTACK_SPEED, _pAtkSpeedAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.PHYSICAL_ATTACK_SPEED, (_pAtkSpeedAmount / 100) + 1);
				}
			}
			
			if (_mAtkSpeedAmount != 0)
			{
				if (_mAtkSpeedMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.MAGIC_ATTACK_SPEED, _mAtkSpeedAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.MAGIC_ATTACK_SPEED, (_mAtkSpeedAmount / 100) + 1);
				}
			}
			
			if (_pAccuracyAmount != 0)
			{
				if (_pAccuracyMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.ACCURACY_COMBAT, _pAccuracyAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.ACCURACY_COMBAT, (_pAccuracyAmount / 100) + 1);
				}
			}
			
			if (_mAccuracyAmount != 0)
			{
				if (_mAccuracyMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.ACCURACY_MAGIC, _mAccuracyAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.ACCURACY_MAGIC, (_mAccuracyAmount / 100) + 1);
				}
			}
			
			if (_pCritRateAmount != 0)
			{
				if (_pCritRateMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.CRITICAL_RATE, _pCritRateAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.CRITICAL_RATE, (_pCritRateAmount / 100) + 1);
				}
			}
			
			if (_mCritRateAmount != 0)
			{
				if (_mCritRateMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.MAGIC_CRITICAL_RATE, _mCritRateAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.MAGIC_CRITICAL_RATE, (_mCritRateAmount / 100) + 1);
				}
			}
			
			if (_pCritDamageAmount != 0)
			{
				if (_pCritDamageMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.CRITICAL_DAMAGE_ADD, _pCritDamageAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.CRITICAL_DAMAGE, (_pCritDamageAmount / 100) + 1);
				}
			}
			
			if (_mCritDamageAmount != 0)
			{
				if (_mCritDamageMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.MAGIC_CRITICAL_DAMAGE_ADD, _mCritDamageAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.MAGIC_CRITICAL_DAMAGE, (_mCritDamageAmount / 100) + 1);
				}
			}
			
			if (_speedAmount != 0)
			{
				if (_speedMode == StatModifierType.DIFF)
				{
					effected.getStat().mergeAdd(Stat.MOVE_SPEED, _speedAmount);
				}
				else // PER
				{
					effected.getStat().mergeMul(Stat.MOVE_SPEED, (_speedAmount / 100) + 1);
				}
			}
		}
	}
}
