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
package handlers.skillconditionhandlers;

import java.util.HashSet;
import java.util.List;
import java.util.Set;

import org.l2jmobius.gameserver.enums.SkillConditionAffectType;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.skill.ISkillCondition;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Mobius
 */
public class OpEquipItemSkillCondition implements ISkillCondition
{
	private final Set<Integer> _itemIds = new HashSet<>();
	private final SkillConditionAffectType _affectType;
	
	public OpEquipItemSkillCondition(StatSet params)
	{
		final List<Integer> itemIds = params.getList("itemIds", Integer.class);
		if (itemIds != null)
		{
			_itemIds.addAll(itemIds);
		}
		else
		{
			_itemIds.add(params.getInt("itemId"));
		}
		_affectType = params.getEnum("affectType", SkillConditionAffectType.class);
	}
	
	@Override
	public boolean canUse(Creature caster, Skill skill, WorldObject target)
	{
		switch (_affectType)
		{
			case CASTER:
			{
				for (int itemId : _itemIds)
				{
					if (caster.getInventory().isItemEquipped(itemId))
					{
						return true;
					}
				}
				break;
			}
			case TARGET:
			{
				if ((target != null) && target.isPlayer())
				{
					for (int itemId : _itemIds)
					{
						if (target.getActingPlayer().getInventory().isItemEquipped(itemId))
						{
							return true;
						}
					}
				}
				break;
			}
			case BOTH:
			{
				if ((target != null) && target.isPlayer())
				{
					for (int itemId : _itemIds)
					{
						if (target.getActingPlayer().getInventory().isItemEquipped(itemId))
						{
							for (int id : _itemIds)
							{
								if (caster.getInventory().isItemEquipped(id))
								{
									return true;
								}
							}
						}
					}
				}
				break;
			}
		}
		return false;
	}
}
