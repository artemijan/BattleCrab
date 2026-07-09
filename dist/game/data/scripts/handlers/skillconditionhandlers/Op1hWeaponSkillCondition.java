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

import java.util.EnumSet;
import java.util.List;
import java.util.Set;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.item.ItemTemplate;
import org.l2jmobius.gameserver.model.item.Weapon;
import org.l2jmobius.gameserver.model.item.type.WeaponType;
import org.l2jmobius.gameserver.model.skill.ISkillCondition;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Mobius
 */
public class Op1hWeaponSkillCondition implements ISkillCondition
{
	private final Set<WeaponType> _weaponTypes = EnumSet.noneOf(WeaponType.class);
	
	public Op1hWeaponSkillCondition(StatSet params)
	{
		final List<String> weaponTypes = params.getList("weaponType", String.class);
		if (weaponTypes != null)
		{
			for (String type : weaponTypes)
			{
				_weaponTypes.add(WeaponType.valueOf(type));
			}
		}
	}
	
	@Override
	public boolean canUse(Creature caster, Skill skill, WorldObject target)
	{
		final Weapon weapon = caster.getActiveWeaponItem();
		if (weapon == null)
		{
			return false;
		}
		
		final WeaponType weaponType = weapon.getItemType();
		for (WeaponType type : _weaponTypes)
		{
			if (type == weaponType)
			{
				return (weapon.getBodyPart() & ItemTemplate.SLOT_LR_HAND) == 0;
			}
		}
		
		return false;
	}
}
