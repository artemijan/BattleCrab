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
package handlers.skill.conditions;

import java.util.HashSet;
import java.util.List;
import java.util.Set;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.skill.ISkillCondition;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.enums.SkillConditionAffectType;

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
						if (target.asPlayer().getInventory().isItemEquipped(itemId))
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
						if (target.asPlayer().getInventory().isItemEquipped(itemId))
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
