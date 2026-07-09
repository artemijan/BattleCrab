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

import java.util.List;

import org.l2jmobius.gameserver.enums.StatModifierType;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.conditions.ConditionUsingItemType;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.type.WeaponType;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * @author Sdw
 */
public class Speed extends AbstractEffect
{
	private final double _amount;
	private final StatModifierType _mode;
	private ConditionUsingItemType _condition;
	
	public Speed(StatSet params)
	{
		_amount = params.getDouble("amount", 0);
		_mode = params.getEnum("mode", StatModifierType.class, StatModifierType.DIFF);
		
		int weaponTypesMask = 0;
		final List<String> weaponTypes = params.getList("weaponType", String.class);
		if (weaponTypes != null)
		{
			for (String weaponType : weaponTypes)
			{
				try
				{
					weaponTypesMask |= WeaponType.valueOf(weaponType).mask();
				}
				catch (IllegalArgumentException e)
				{
					final IllegalArgumentException exception = new IllegalArgumentException("weaponType should contain WeaponType enum value but found " + weaponType);
					exception.addSuppressed(e);
					throw exception;
				}
			}
		}
		if (weaponTypesMask != 0)
		{
			_condition = new ConditionUsingItemType(weaponTypesMask);
		}
	}
	
	@Override
	public void pump(Creature effected, Skill skill)
	{
		if (_condition == null || _condition.test(effected, effected, skill))
		{
			switch (_mode)
			{
				case DIFF:
				{
					effected.getStat().mergeAdd(Stat.RUN_SPEED, _amount);
					effected.getStat().mergeAdd(Stat.WALK_SPEED, _amount);
					effected.getStat().mergeAdd(Stat.SWIM_RUN_SPEED, _amount);
					effected.getStat().mergeAdd(Stat.SWIM_WALK_SPEED, _amount);
					effected.getStat().mergeAdd(Stat.FLY_RUN_SPEED, _amount);
					effected.getStat().mergeAdd(Stat.FLY_WALK_SPEED, _amount);
					break;
				}
				case PER:
				{
					effected.getStat().mergeMul(Stat.RUN_SPEED, (_amount / 100) + 1);
					effected.getStat().mergeMul(Stat.WALK_SPEED, (_amount / 100) + 1);
					effected.getStat().mergeMul(Stat.SWIM_RUN_SPEED, (_amount / 100) + 1);
					effected.getStat().mergeMul(Stat.SWIM_WALK_SPEED, (_amount / 100) + 1);
					effected.getStat().mergeMul(Stat.FLY_RUN_SPEED, (_amount / 100) + 1);
					effected.getStat().mergeMul(Stat.FLY_WALK_SPEED, (_amount / 100) + 1);
					break;
				}
			}
		}
	}
}
