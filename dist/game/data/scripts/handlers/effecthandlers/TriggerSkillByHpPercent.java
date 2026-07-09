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

import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.enums.SkillFinishType;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.impl.creature.OnCreatureHpChange;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;

/**
 * @author Mobius
 */
public class TriggerSkillByHpPercent extends AbstractEffect
{
	private final int _skillId;
	private final int _skillLevel;
	private final int _percentFrom;
	private final int _percentTo;
	
	public TriggerSkillByHpPercent(StatSet params)
	{
		_skillId = params.getInt("skillId", 0);
		_skillLevel = params.getInt("skillLevel", 1);
		_percentFrom = params.getInt("percentFrom", 0);
		_percentTo = params.getInt("percentTo", 100);
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		effected.addListener(new ConsumerEventListener(effected, EventType.ON_CREATURE_HP_CHANGE, (OnCreatureHpChange event) -> onHpChange(event), this));
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		effected.removeListenerIf(EventType.ON_CREATURE_HP_CHANGE, listener -> listener.getOwner() == this);
	}
	
	private void onHpChange(OnCreatureHpChange event)
	{
		final Creature creature = event.getCreature();
		final int hpPercent = creature.getCurrentHpPercent();
		if ((hpPercent >= _percentFrom) && (hpPercent <= _percentTo))
		{
			if (!creature.isAffectedBySkill(_skillId))
			{
				SkillCaster.triggerCast(creature, creature, SkillData.getInstance().getSkill(_skillId, _skillLevel));
			}
		}
		else
		{
			creature.getEffectList().stopSkillEffects(SkillFinishType.REMOVED, _skillId);
		}
	}
}
