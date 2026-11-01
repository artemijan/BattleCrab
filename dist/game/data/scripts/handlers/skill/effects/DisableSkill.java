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

import java.util.Collections;
import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Ofelin
 */
public class DisableSkill extends AbstractEffect
{
	private final Set<Integer> _disabledSkills;
	
	public DisableSkill(StatSet params)
	{
		final String disable = params.getString("disable");
		if ((disable != null) && !disable.isEmpty())
		{
			_disabledSkills = new HashSet<>();
			for (String slot : disable.split(";"))
			{
				_disabledSkills.add(Integer.parseInt(slot));
			}
		}
		else
		{
			_disabledSkills = Collections.emptySet();
		}
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		Skill knownSkill;
		for (int disableSkillId : _disabledSkills)
		{
			knownSkill = effected.getKnownSkill(disableSkillId);
			if (knownSkill != null)
			{
				effected.disableSkill(knownSkill, 0);
			}
		}
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		Skill knownSkill;
		for (int enableSkillId : _disabledSkills)
		{
			knownSkill = effected.getKnownSkill(enableSkillId);
			if (knownSkill != null)
			{
				if (effected.isPlayer())
				{
					effected.asPlayer().enableSkill(knownSkill, false);
				}
				else
				{
					effected.enableSkill(knownSkill);
				}
			}
		}
	}
}
