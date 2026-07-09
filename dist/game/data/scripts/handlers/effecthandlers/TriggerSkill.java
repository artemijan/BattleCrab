/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package handlers.effecthandlers;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.handler.TargetHandler;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.holders.SkillHolder;
import org.l2jmobius.gameserver.model.holders.SkillUseHolder;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
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
		
		final Skill triggerSkill = _adjustLevel ? SkillData.getInstance().getSkill(_skill.getSkillId(), skill.getLevel()) : _skill.getSkill();
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
		
		final SkillUseHolder queuedSkill = effector.getActingPlayer().getQueuedSkill();
		if (queuedSkill != null)
		{
			ThreadPool.schedule(() ->
			{
				effector.getActingPlayer().setQueuedSkill(queuedSkill.getSkill(), queuedSkill.getItem(), queuedSkill.isCtrlPressed(), queuedSkill.isShiftPressed());
			}, 10);
		}
		
		effector.getActingPlayer().setQueuedSkill(triggerSkill, null, false, false);
	}
}
