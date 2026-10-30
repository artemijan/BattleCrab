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
package handlers.skill.conditions;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.skill.ISkillCondition;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.BaseStat;

/**
 * @author Mobius
 */
public class OpBaseStatSkillCondition implements ISkillCondition
{
	private final BaseStat _stat;
	private final int _min;
	private final int _max;
	
	public OpBaseStatSkillCondition(StatSet params)
	{
		_stat = params.getEnum("stat", BaseStat.class);
		_min = params.getInt("min", 0);
		_max = params.getInt("max", 2147483647);
	}
	
	@Override
	public boolean canUse(Creature caster, Skill skill, WorldObject target)
	{
		int currentValue = 0;
		switch (_stat)
		{
			case STR:
			{
				currentValue = caster.getSTR();
				break;
			}
			case INT:
			{
				currentValue = caster.getINT();
				break;
			}
			case DEX:
			{
				currentValue = caster.getDEX();
				break;
			}
			case WIT:
			{
				currentValue = caster.getWIT();
				break;
			}
			case CON:
			{
				currentValue = caster.getCON();
				break;
			}
			case MEN:
			{
				currentValue = caster.getMEN();
				break;
			}
		}
		
		return (currentValue >= _min) && (currentValue <= _max);
	}
}
