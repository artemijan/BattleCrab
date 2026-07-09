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
package handlers.skillconditionhandlers;

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
