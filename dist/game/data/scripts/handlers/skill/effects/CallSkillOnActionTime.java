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

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * Dam Over Time effect implementation.
 */
public class CallSkillOnActionTime extends AbstractEffect
{
	private final SkillHolder _skill;
	
	public CallSkillOnActionTime(StatSet params)
	{
		_skill = new SkillHolder(params.getInt("skillId"), params.getInt("skillLevel", 1), params.getInt("skillSubLevel", 0));
		setTicks(params.getInt("ticks"));
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (!_skill.getSkill().isSynergySkill())
		{
			effected.getEffectList().stopEffects(Collections.singleton(_skill.getSkill().getAbnormalType()));
			effected.getEffectList().addBlockedAbnormalTypes(Collections.singleton(_skill.getSkill().getAbnormalType()));
		}
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		if (!_skill.getSkill().isSynergySkill())
		{
			effected.getEffectList().removeBlockedAbnormalTypes(Collections.singleton(_skill.getSkill().getAbnormalType()));
		}
	}
	
	@Override
	public boolean onActionTime(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effector.isDead())
		{
			return false;
		}
		
		final Skill triggerSkill = _skill.getSkill();
		if (triggerSkill != null)
		{
			if (triggerSkill.isSynergySkill())
			{
				triggerSkill.applyEffects(effector, effector);
			}
			
			World.getInstance().forEachVisibleObjectInRange(effector, Creature.class, _skill.getSkill().getAffectRange(), c ->
			{
				final WorldObject target = triggerSkill.getTarget(effector, c, false, false, false);
				if ((target != null) && target.isCreature())
				{
					SkillCaster.triggerCast(effector, target.asCreature(), triggerSkill);
				}
			});
		}
		else
		{
			LOGGER.warning("Skill not found effect called from " + skill);
		}
		
		return skill.isToggle();
	}
}
