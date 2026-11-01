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

import java.util.List;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.config.custom.CancelReturnConfig;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.effects.EffectType;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.enums.DispelSlotType;
import org.l2jmobius.gameserver.model.skill.enums.SkillFinishType;
import org.l2jmobius.gameserver.model.stats.Formulas;

/**
 * Dispel By Category effect implementation.
 * @author DS, Adry_85, Naker
 */
public class DispelByCategory extends AbstractEffect
{
	private final DispelSlotType _slot;
	private final int _rate;
	private final int _max;
	
	public DispelByCategory(StatSet params)
	{
		_slot = params.getEnum("slot", DispelSlotType.class, DispelSlotType.BUFF);
		_rate = params.getInt("rate", 0);
		_max = params.getInt("max", 0);
	}
	
	@Override
	public EffectType getEffectType()
	{
		return EffectType.DISPEL;
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if ((skill == null) || (effected == null) || effected.isDead() || effected.isRaid())
		{
			return;
		}
		
		if (!CancelReturnConfig.CANCEL_RETURN_ON)
		{
			normalCancel(effector, effected, skill);
			return;
		}
		
		if ((effector.isPlayer() && !CancelReturnConfig.CANCEL_RETURN_PLAYER) || ((effector.isMonster() || effector.isRaid()) && !CancelReturnConfig.CANCEL_RETURN_MOB))
		{
			normalCancel(effector, effected, skill);
			return;
		}
		
		if (!CancelReturnConfig.CANCEL_RETURN_PLAYER_OLYS && effected.isPlayer() && effected.asPlayer().isInOlympiadMode())
		{
			normalCancel(effector, effected, skill);
			return;
		}
		
		final List<BuffInfo> canceled = Formulas.calcCancelStealEffects(effector, effected, skill, _slot, _rate, _max);
		if (canceled.isEmpty())
		{
			return;
		}
		
		for (BuffInfo info : canceled)
		{
			effected.getEffectList().stopSkillEffects(SkillFinishType.REMOVED, info.getSkill());
		}
		
		ThreadPool.schedule(() ->
		{
			if ((!effected.asPlayer().isOnline()) || effected.isDead() || effected.isMonster())
			{
				return;
			}
			
			for (BuffInfo info : canceled)
			{
				final Skill sk = info.getSkill();
				final int timeLeft = info.getTime();
				
				if ((sk == null) || (timeLeft <= 0))
				{
					continue;
				}
				
				if (effected.getEffectList().isAffectedBySkill(sk.getId()))
				{
					continue;
				}
				
				sk.applyEffects(effected, effected);
				
				final BuffInfo newInfo = effected.getEffectList().getBuffInfoBySkillId(sk.getId());
				if (newInfo != null)
				{
					newInfo.setAbnormalTime(timeLeft);
				}
			}
			
		}, CancelReturnConfig.TIME_TO_RETURN);
	}
	
	private void normalCancel(Creature effector, Creature effected, Skill skill)
	{
		final List<BuffInfo> canceled = Formulas.calcCancelStealEffects(effector, effected, skill, _slot, _rate, _max);
		for (BuffInfo info : canceled)
		{
			effected.getEffectList().stopSkillEffects(SkillFinishType.REMOVED, info.getSkill());
		}
	}
}
