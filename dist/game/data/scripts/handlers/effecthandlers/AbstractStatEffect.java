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

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.enums.StatModifierType;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.conditions.Condition;
import org.l2jmobius.gameserver.model.conditions.ConditionPlayerIsInCombat;
import org.l2jmobius.gameserver.model.conditions.ConditionUsingItemType;
import org.l2jmobius.gameserver.model.conditions.ConditionUsingMagicWeapon;
import org.l2jmobius.gameserver.model.conditions.ConditionUsingTwoHandWeapon;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.type.ArmorType;
import org.l2jmobius.gameserver.model.item.type.WeaponType;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * @author Sdw, Mobius
 */
public abstract class AbstractStatEffect extends AbstractEffect
{
	protected final Stat _addStat;
	protected final Stat _mulStat;
	protected final double _amount;
	protected final StatModifierType _mode;
	protected final List<Condition> _conditions = new ArrayList<>();
	
	public AbstractStatEffect(StatSet params, Stat stat)
	{
		this(params, stat, stat);
	}
	
	public AbstractStatEffect(StatSet params, Stat mulStat, Stat addStat)
	{
		_addStat = addStat;
		_mulStat = mulStat;
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
		
		int armorTypesMask = 0;
		final List<String> armorTypes = params.getList("armorType", String.class);
		if (armorTypes != null)
		{
			for (String armorType : armorTypes)
			{
				try
				{
					armorTypesMask |= ArmorType.valueOf(armorType).mask();
				}
				catch (IllegalArgumentException e)
				{
					final IllegalArgumentException exception = new IllegalArgumentException("armorTypes should contain ArmorType enum value but found " + armorType);
					exception.addSuppressed(e);
					throw exception;
				}
			}
		}
		
		if (weaponTypesMask != 0)
		{
			_conditions.add(new ConditionUsingItemType(weaponTypesMask));
		}
		
		if (armorTypesMask != 0)
		{
			_conditions.add(new ConditionUsingItemType(armorTypesMask));
		}
		
		if (params.contains("inCombat"))
		{
			_conditions.add(new ConditionPlayerIsInCombat(params.getBoolean("inCombat")));
		}
		
		if (params.contains("magicWeapon"))
		{
			_conditions.add(new ConditionUsingMagicWeapon(params.getBoolean("magicWeapon")));
		}
		
		if (params.contains("twoHandWeapon"))
		{
			_conditions.add(new ConditionUsingTwoHandWeapon(params.getBoolean("twoHandWeapon")));
		}
	}
	
	@Override
	public void pump(Creature effected, Skill skill)
	{
		if (!_conditions.isEmpty())
		{
			for (Condition cond : _conditions)
			{
				if (!cond.test(effected, effected, skill))
				{
					return;
				}
			}
		}
		
		switch (_mode)
		{
			case DIFF:
			{
				effected.getStat().mergeAdd(_addStat, _amount);
				break;
			}
			case PER:
			{
				effected.getStat().mergeMul(_mulStat, (_amount / 100) + 1);
				break;
			}
		}
	}
}
