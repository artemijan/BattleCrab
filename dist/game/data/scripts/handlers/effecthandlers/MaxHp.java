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

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * @author NosBit
 */
public class MaxHp extends AbstractStatEffect
{
	private final boolean _heal;
	
	public MaxHp(StatSet params)
	{
		super(params, Stat.MAX_HP);
		
		_heal = params.getBoolean("heal", false);
	}
	
	@Override
	public void continuousInstant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (_heal)
		{
			ThreadPool.schedule(() ->
			{
				if (!effected.isHpBlocked())
				{
					switch (_mode)
					{
						case DIFF:
						{
							effected.setCurrentHp(effected.getCurrentHp() + _amount);
							break;
						}
						case PER:
						{
							effected.setCurrentHp(effected.getCurrentHp() + (effected.getMaxHp() * (_amount / 100)));
							break;
						}
					}
				}
			}, 100);
		}
	}
}
