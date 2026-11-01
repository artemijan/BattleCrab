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
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.AbnormalType;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.enums.SkillFinishType;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * Synergy effect implementation.
 */
public class Synergy extends AbstractEffect
{
	private final Set<AbnormalType> _requiredSlots;
	private final Set<AbnormalType> _optionalSlots;
	private final int _partyBuffSkillId;
	private final int _skillLevelScaleTo;
	private final int _minSlot;
	
	public Synergy(StatSet params)
	{
		final String requiredSlots = params.getString("requiredSlots", null);
		if ((requiredSlots != null) && !requiredSlots.isEmpty())
		{
			_requiredSlots = new HashSet<>();
			for (String slot : requiredSlots.split(";"))
			{
				_requiredSlots.add(AbnormalType.getAbnormalType(slot));
			}
		}
		else
		{
			_requiredSlots = Collections.<AbnormalType> emptySet();
		}
		
		final String optionalSlots = params.getString("optionalSlots", null);
		if ((optionalSlots != null) && !optionalSlots.isEmpty())
		{
			_optionalSlots = new HashSet<>();
			for (String slot : optionalSlots.split(";"))
			{
				_optionalSlots.add(AbnormalType.getAbnormalType(slot));
			}
		}
		else
		{
			_optionalSlots = Collections.<AbnormalType> emptySet();
		}
		
		_partyBuffSkillId = params.getInt("partyBuffSkillId");
		_skillLevelScaleTo = params.getInt("skillLevelScaleTo", 1);
		_minSlot = params.getInt("minSlot", 2);
		setTicks(params.getInt("ticks"));
	}
	
	@Override
	public boolean onActionTime(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effector.isDead())
		{
			return false;
		}
		
		for (AbnormalType required : _requiredSlots)
		{
			if (!effector.hasAbnormalType(required))
			{
				return skill.isToggle();
			}
		}
		
		int abnormalCount = 0;
		for (AbnormalType abnormalType : _optionalSlots)
		{
			if (effector.hasAbnormalType(abnormalType))
			{
				abnormalCount++;
			}
		}
		
		if (abnormalCount >= _minSlot)
		{
			final SkillHolder partyBuff = new SkillHolder(_partyBuffSkillId, Math.min(abnormalCount - 1, _skillLevelScaleTo));
			final Skill partyBuffSkill = partyBuff.getSkill();
			if (partyBuffSkill != null)
			{
				final WorldObject target = partyBuffSkill.getTarget(effector, effected, false, false, false);
				if ((target != null) && target.isCreature())
				{
					final BuffInfo abnormalBuffInfo = effector.getEffectList().getFirstBuffInfoByAbnormalType(partyBuffSkill.getAbnormalType());
					if ((abnormalBuffInfo != null) && (abnormalBuffInfo.getSkill().getAbnormalLevel() != (abnormalCount - 1)))
					{
						effector.getEffectList().stopSkillEffects(SkillFinishType.REMOVED, _partyBuffSkillId);
					}
					else
					{
						SkillCaster.triggerCast(effector, target.asCreature(), partyBuffSkill);
					}
				}
			}
			else
			{
				LOGGER.warning("Skill not found effect called from " + skill);
			}
		}
		else
		{
			effector.getEffectList().stopSkillEffects(SkillFinishType.REMOVED, _partyBuffSkillId);
		}
		
		return skill.isToggle();
	}
}
