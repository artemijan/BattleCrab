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

import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.handler.TargetHandler;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.skill.targets.TargetType;

/**
 * Trigger Skill effect implementation.
 * @author Mobius
 */
public class TriggerSkill extends AbstractEffect
{
	private final SkillHolder _skill;
	private final TargetType _targetType;
	private final boolean _adjustLevel;
	
	public TriggerSkill(StatSet params)
	{
		_skill = new SkillHolder(params.getInt("skillId"), params.getInt("skillLevel", 1));
		_targetType = params.getEnum("targetType", TargetType.class, TargetType.TARGET);
		_adjustLevel = params.getBoolean("adjustLevel", false);
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if ((effected == null) || !effected.isCreature() || !effector.isPlayer())
		{
			return;
		}
		
		final Skill triggerSkill = _adjustLevel ? SkillData.getInstance().getSkill(_skill.getSkillId(), skill.getLevel(), skill.getSubLevel()) : _skill.getSkill();
		if (triggerSkill == null)
		{
			return;
		}
		
		WorldObject target = null;
		try
		{
			target = TargetHandler.getInstance().getHandler(_targetType).getTarget(effector, effected, triggerSkill, false, false, false);
		}
		catch (Exception e)
		{
			LOGGER.warning("Exception in ITargetTypeHandler.getTarget(): " + e.getMessage());
		}
		
		if ((target == null) || !target.isCreature())
		{
			return;
		}
		
		// final SkillUseHolder queuedSkill = effector.asPlayer().getQueuedSkill();
		// if (queuedSkill != null)
		// {
		// ThreadPool.schedule(() -> effector.asPlayer().setQueuedSkill(queuedSkill.getSkill(), queuedSkill.getItem(), queuedSkill.isCtrlPressed(), queuedSkill.isShiftPressed()), 10);
		// }
		// effector.asPlayer().setQueuedSkill(triggerSkill, null, false, false);
		
		// Removed above delayed queuedSkill code. Just trigger cast.
		SkillCaster.triggerCast(effector, effected, _skill.getSkill());
	}
}
