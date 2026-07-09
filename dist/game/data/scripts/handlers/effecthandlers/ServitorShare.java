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

import java.util.EnumMap;
import java.util.Map;
import java.util.Map.Entry;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * Servitor Share effect implementation.
 */
public class ServitorShare extends AbstractEffect
{
	private final Map<Stat, Float> _sharedStats = new EnumMap<>(Stat.class);
	
	public ServitorShare(StatSet params)
	{
		if (params.isEmpty())
		{
			return;
		}
		
		for (Entry<String, Object> param : params.getSet().entrySet())
		{
			_sharedStats.put(Stat.valueOf(param.getKey()), (Float.parseFloat((String) param.getValue())) / 100);
		}
	}
	
	@Override
	public boolean canPump(Creature effector, Creature effected, Skill skill)
	{
		return effected.isSummon();
	}
	
	@Override
	public void pump(Creature effected, Skill skill)
	{
		final Player owner = effected.getActingPlayer();
		if (owner != null)
		{
			for (Entry<Stat, Float> stats : _sharedStats.entrySet())
			{
				effected.getStat().mergeAdd(stats.getKey(), owner.getStat().getValue(stats.getKey()) * stats.getValue().floatValue());
			}
		}
	}
}