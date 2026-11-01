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
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.stat.PlayerStat;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.zone.ZoneId;

/**
 * Recover Vitality in Peace Zone effect implementation.
 * @author Mobius
 */
public class RecoverVitalityInPeaceZone extends AbstractEffect
{
	private final double _amount;
	private final int _ticks;
	
	public RecoverVitalityInPeaceZone(StatSet params)
	{
		_amount = params.getDouble("amount", 0);
		_ticks = params.getInt("ticks", 10);
	}
	
	@Override
	public int getTicks()
	{
		return _ticks;
	}
	
	@Override
	public boolean onActionTime(Creature effector, Creature effected, Skill skill, Item item)
	{
		if ((effected == null) || effected.isDead() || !effected.isPlayer() || !effected.isInsideZone(ZoneId.PEACE))
		{
			return false;
		}
		
		final Player target = effected.asPlayer();
		long vitality = target.getVitalityPoints();
		vitality += _amount;
		if (vitality >= PlayerStat.MAX_VITALITY_POINTS)
		{
			vitality = PlayerStat.MAX_VITALITY_POINTS;
		}
		
		target.setVitalityPoints((int) vitality, true);
		
		return skill.isToggle();
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		if ((effected != null) && effected.isPlayer())
		{
			final BuffInfo info = effected.getEffectList().getBuffInfoBySkillId(skill.getId());
			if ((info != null) && !info.isRemoved())
			{
				final Player target = effected.asPlayer();
				long vitality = target.getVitalityPoints();
				vitality += _amount * 100;
				if (vitality >= PlayerStat.MAX_VITALITY_POINTS)
				{
					vitality = PlayerStat.MAX_VITALITY_POINTS;
				}
				
				target.setVitalityPoints((int) vitality, true);
			}
		}
	}
}
