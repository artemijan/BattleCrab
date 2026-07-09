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

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.conditions.Condition;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * @author Sdw, Mobius
 */
public class CriticalRate extends AbstractConditionalHpEffect
{
	public CriticalRate(StatSet params)
	{
		super(params, Stat.CRITICAL_RATE);
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
				effected.getStat().mergeMul(_mulStat, (_amount / 100));
				break;
			}
		}
	}
}
