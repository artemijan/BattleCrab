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

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.enums.creature.Position;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;
import org.l2jmobius.gameserver.util.MathUtil;

/**
 * @author Sdw, Mobius
 */
public class CriticalRatePositionBonus extends AbstractEffect
{
	private final double _amount;
	private final Position _position;
	
	public CriticalRatePositionBonus(StatSet params)
	{
		_amount = params.getDouble("amount", 0);
		_position = params.getEnum("position", Position.class, Position.FRONT);
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		effected.getStat().mergePositionTypeValue(Stat.CRITICAL_RATE, _position, (_amount / 100) + 1, MathUtil::mul);
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		// Handle 100% resistance case explicitly.
		final double amount = (_amount / 100) + 1;
		if (amount == 0d)
		{
			// To reverse 100% resistance, reset to the default value.
			effected.getStat().mergePositionTypeValue(Stat.CRITICAL_RATE, _position, 1d, (existing, ignored) -> 1d);
			
			// Recalculate stats in case effected has other instances of this effect.
			ThreadPool.schedule(() ->
			{
				if (effected.isSpawned())
				{
					effected.getStat().recalculateStats(true);
				}
			}, 1000);
		}
		else // For other cases, proceed with normal division to remove the effect.
		{
			effected.getStat().mergePositionTypeValue(Stat.CRITICAL_RATE, _position, amount, MathUtil::div);
		}
	}
}
