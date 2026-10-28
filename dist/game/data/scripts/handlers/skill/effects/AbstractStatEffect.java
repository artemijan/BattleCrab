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
package handlers.skill.effects;

import java.util.ArrayList;
import java.util.List;

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
import org.l2jmobius.gameserver.model.skill.enums.StatModifierType;
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
		this(params, stat, stat, false);
	}
	
	public AbstractStatEffect(StatSet params, Stat mulStat, Stat addStat)
	{
		this(params, mulStat, addStat, false);
	}
	
	public AbstractStatEffect(StatSet params, Stat mulStat, Stat addStat, boolean negativePercent)
	{
		_addStat = addStat;
		_mulStat = mulStat;
		_mode = params.getEnum("mode", StatModifierType.class, StatModifierType.DIFF);
		if (negativePercent && (_mode == StatModifierType.PER))
		{
			_amount = params.getDouble("amount", 0) * -1d;
		}
		else
		{
			_amount = params.getDouble("amount", 0);
		}
		
		if (params.contains("power"))
		{
			throw new IllegalArgumentException(getClass().getSimpleName() + " should use amount instead of power.");
		}
		
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
