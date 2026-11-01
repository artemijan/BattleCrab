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

import java.util.HashSet;
import java.util.LinkedList;
import java.util.List;
import java.util.Set;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.config.custom.CancelReturnConfig;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.holders.creature.EffectList;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.effects.EffectType;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.AbnormalType;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * Dispel By Slot Probability effect implementation.
 * @author Adry_85, Zoey76, Naker
 */
public class DispelBySlotProbability extends AbstractEffect
{
	private final Set<AbnormalType> _dispelAbnormals;
	private final int _rate;
	
	public DispelBySlotProbability(StatSet params)
	{
		final String[] dispelEffects = params.getString("dispel").split(";");
		_rate = params.getInt("rate", 100);
		_dispelAbnormals = new HashSet<>(dispelEffects.length);
		
		for (String slot : dispelEffects)
		{
			_dispelAbnormals.add(Enum.valueOf(AbnormalType.class, slot));
		}
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
		if ((effected == null) || effected.isRaid())
		{
			return;
		}
		
		if (!CancelReturnConfig.CANCEL_RETURN_ON)
		{
			normalCancel(effected);
			return;
		}
		
		if ((effector.isPlayer() && !CancelReturnConfig.CANCEL_RETURN_PLAYER) || ((effector.isMonster() && !CancelReturnConfig.CANCEL_RETURN_MOB) || ((effector.isRaid()) && !CancelReturnConfig.CANCEL_RETURN_MOB)))
		{
			normalCancel(effected);
			return;
		}
		
		if (!CancelReturnConfig.CANCEL_RETURN_PLAYER_OLYS && effected.isPlayer() && effected.asPlayer().isInOlympiadMode())
		{
			normalCancel(effected);
			return;
		}
		
		final EffectList effectList = effected.getEffectList();
		final List<BuffInfo> canceled = new LinkedList<>();
		
		effectList.stopEffects(info ->
		{
			final boolean cancel = !info.getSkill().isIrreplaceableBuff() && (Rnd.get(100) < _rate) && _dispelAbnormals.contains(info.getSkill().getAbnormalType());
			if (cancel)
			{
				canceled.add(info);
			}
			return cancel;
		}, true, true);
		
		if (canceled.isEmpty())
		{
			return;
		}
		
		ThreadPool.schedule(() ->
		{
			if ((!effected.asPlayer().isOnline()) || effected.isDead() || effected.isMonster())
			{
				return;
			}
			
			for (BuffInfo oldInfo : canceled)
			{
				final Skill sk = oldInfo.getSkill();
				final int timeLeft = oldInfo.getTime();
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
	
	private void normalCancel(Creature effected)
	{
		effected.getEffectList().stopEffects(info -> !info.getSkill().isIrreplaceableBuff() && (Rnd.get(100) < _rate) && _dispelAbnormals.contains(info.getSkill().getAbnormalType()), true, true);
	}
}
