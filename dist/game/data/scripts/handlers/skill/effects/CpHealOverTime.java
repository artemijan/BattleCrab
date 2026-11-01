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

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * Cp Heal Over Time effect implementation.
 */
public class CpHealOverTime extends AbstractEffect
{
	private final double _power;
	
	public CpHealOverTime(StatSet params)
	{
		_power = params.getDouble("power", 0);
		setTicks(params.getInt("ticks"));
		
		if (params.contains("amount"))
		{
			throw new IllegalArgumentException(getClass().getSimpleName() + " should use power instead of amount.");
		}
	}
	
	@Override
	public boolean onActionTime(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effected.isDead())
		{
			return false;
		}
		
		double cp = effected.getCurrentCp();
		final double maxcp = effected.getMaxRecoverableCp();
		
		// Not needed to set the CP and send update packet if player is already at max CP
		if (_power > 0)
		{
			if (cp >= maxcp)
			{
				return false;
			}
		}
		else
		{
			if ((cp - _power) <= 0)
			{
				return false;
			}
		}
		
		double power = _power;
		if ((item != null) && (item.isPotion() || item.isElixir()))
		{
			power += effected.getStat().getValue(Stat.ADDITIONAL_POTION_CP, 0) / getTicks();
		}
		
		cp += power * getTicksMultiplier();
		if (_power > 0)
		{
			cp = Math.min(cp, maxcp);
		}
		else
		{
			cp = Math.max(cp, 1);
		}
		
		effected.setCurrentCp(cp, false);
		effected.broadcastStatusUpdate(effector);
		return true;
	}
}
