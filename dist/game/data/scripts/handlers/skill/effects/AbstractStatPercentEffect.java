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
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.enums.StatModifierType;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * @author Mobius
 */
public class AbstractStatPercentEffect extends AbstractEffect
{
	private final Stat _stat;
	protected final double _amount;
	
	public AbstractStatPercentEffect(StatSet params, Stat stat)
	{
		_stat = stat;
		_amount = params.getDouble("amount", 1);
		
		if (params.getEnum("mode", StatModifierType.class, StatModifierType.PER) != StatModifierType.PER)
		{
			throw new IllegalArgumentException(getClass().getSimpleName() + " can only use PER mode.");
		}
		
		if (params.contains("power"))
		{
			throw new IllegalArgumentException(getClass().getSimpleName() + " should use amount instead of power.");
		}
	}
	
	@Override
	public void pump(Creature effected, Skill skill)
	{
		effected.getStat().mergeMul(_stat, (_amount / 100) + 1);
	}
}
