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

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.holders.TemplateChanceHolder;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * Transformation type effect, which disables attack or use of skills.
 * @author Nik, Mobius
 */
public class ChangeBody extends AbstractEffect
{
	private final Set<TemplateChanceHolder> _transformations = new HashSet<>();
	
	public ChangeBody(StatSet params)
	{
		for (StatSet item : params.getList("templates", StatSet.class))
		{
			_transformations.add(new TemplateChanceHolder(item.getInt(".templateId"), item.getInt(".minChance"), item.getInt(".maxChance")));
		}
	}
	
	@Override
	public boolean canStart(Creature effector, Creature effected, Skill skill)
	{
		return !effected.isDoor();
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		final int chance = Rnd.get(100);
		for (TemplateChanceHolder holder : _transformations)
		{
			if (holder.calcChance(chance))
			{
				effected.transform(holder.getTemplateId(), false);
				return;
			}
		}
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		effected.stopTransformation(false);
	}
}
