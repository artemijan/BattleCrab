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

import java.util.EnumMap;
import java.util.Map;
import java.util.Map.Entry;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.stat.CreatureStat;
import org.l2jmobius.gameserver.model.actor.stat.PlayerStat;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * Servitor Share effect implementation.
 * @author Mobius
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
		final Player player = effected.asPlayer();
		if (player == null)
		{
			return;
		}
		
		final PlayerStat playerStat = player.getStat();
		final CreatureStat effectedStat = effected.getStat();
		for (Entry<Stat, Float> entry : _sharedStats.entrySet())
		{
			final Stat stat = entry.getKey();
			effectedStat.mergeAdd(stat, playerStat.getValue(stat) * entry.getValue());
		}
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		if (effected.isSummon())
		{
			effected.getStat().recalculateStats(true);
			return;
		}
		
		final Player player = effected.asPlayer();
		if (player == null)
		{
			return;
		}
		
		player.getServitors().values().forEach(s -> s.getStat().recalculateStats(true));
	}
}
