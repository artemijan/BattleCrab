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

import java.util.List;

import org.l2jmobius.gameserver.enums.SkillConditionAffectType;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.skill.ISkillCondition;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Mobius
 */
public class OpCheckSkillListSkillCondition implements ISkillCondition
{
	private final List<Integer> _skillIds;
	private final SkillConditionAffectType _affectType;
	
	public OpCheckSkillListSkillCondition(StatSet params)
	{
		_skillIds = params.getList("skillIds", Integer.class);
		_affectType = params.getEnum("affectType", SkillConditionAffectType.class);
	}
	
	@Override
	public boolean canUse(Creature caster, Skill skill, WorldObject target)
	{
		switch (_affectType)
		{
			case CASTER:
			{
				for (int id : _skillIds)
				{
					if (caster.getSkillLevel(id) > 0)
					{
						return true;
					}
				}
				break;
			}
			case TARGET:
			{
				if ((target != null) && !target.isPlayer())
				{
					final Player player = target.getActingPlayer();
					for (int id : _skillIds)
					{
						if (player.getSkillLevel(id) > 0)
						{
							return true;
						}
					}
				}
				break;
			}
		}
		return false;
	}
}
