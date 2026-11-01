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

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * @author Mobius
 */
public class TriggerSkillWithDelay extends AbstractEffect
{
	private final List<SkillHolder> _triggerSkill;
	private final List<Integer> _delay;
	
	public TriggerSkillWithDelay(StatSet params)
	{
		final String triggerSkills = params.getString("triggerSkill", "");
		if (triggerSkills.isEmpty())
		{
			_triggerSkill = Collections.emptyList();
		}
		else
		{
			final String[] split = triggerSkills.split(";");
			_triggerSkill = new ArrayList<>(split.length);
			
			for (String skill : split)
			{
				final String[] splitSkill = skill.split(",");
				if (splitSkill.length == 1)
				{
					_triggerSkill.add(new SkillHolder(Integer.parseInt(splitSkill[0]), 1));
				}
				else
				{
					_triggerSkill.add(new SkillHolder(Integer.parseInt(splitSkill[0]), Integer.parseInt(splitSkill[1])));
				}
			}
		}
		
		final String delays = params.getString("delay", "");
		if (triggerSkills.isEmpty())
		{
			final int size = _triggerSkill.size();
			_delay = new ArrayList<>(size);
			for (int i = 0; i < size; i++)
			{
				_delay.add(1000);
			}
		}
		else
		{
			final String[] split = delays.split(";");
			if (split.length == 1)
			{
				final int size = _triggerSkill.size();
				_delay = new ArrayList<>(size);
				
				final int delay = Integer.parseInt(split[0]);
				for (int i = 0; i < size; i++)
				{
					_delay.add(delay);
				}
			}
			else
			{
				_delay = new ArrayList<>(split.length);
				for (String delay : split)
				{
					_delay.add(Integer.parseInt(delay));
				}
			}
		}
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		int delay = 0;
		int index = 0;
		for (SkillHolder holder : _triggerSkill)
		{
			delay += _delay.get(index++);
			ThreadPool.schedule(() -> SkillCaster.triggerCast(effector, effected, holder.getSkill()), delay);
		}
	}
}
