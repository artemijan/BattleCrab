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

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.item.ItemTemplate;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.item.type.ArmorType;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.skill.ISkillCondition;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Sdw
 */
public class EquipArmorSkillCondition implements ISkillCondition
{
	private int _armorTypesMask = 0;
	
	public EquipArmorSkillCondition(StatSet params)
	{
		final List<ArmorType> armorTypes = params.getEnumList("armorType", ArmorType.class);
		if (armorTypes != null)
		{
			for (ArmorType armorType : armorTypes)
			{
				_armorTypesMask |= armorType.mask();
			}
		}
	}
	
	@Override
	public boolean canUse(Creature caster, Skill skill, WorldObject target)
	{
		if ((caster == null) || !caster.isPlayer())
		{
			return false;
		}
		
		// Get the itemMask of the weared chest (if exists).
		final Inventory inv = caster.getInventory();
		final Item chest = inv.getPaperdollItem(Inventory.PAPERDOLL_CHEST);
		if (chest == null)
		{
			return false;
		}
		
		// If chest armor is different from the condition one return false.
		final int chestMask = chest.getTemplate().getItemMask();
		if ((_armorTypesMask & chestMask) == 0)
		{
			return false;
		}
		
		// So from here, chest armor matches conditions.
		
		// Return true if chest armor is a Full Armor.
		final int chestBodyPart = chest.getTemplate().getBodyPart();
		if (chestBodyPart == ItemTemplate.SLOT_FULL_ARMOR)
		{
			return true;
		}
		
		// Check legs armor.
		final Item legs = inv.getPaperdollItem(Inventory.PAPERDOLL_LEGS);
		if (legs == null)
		{
			return false;
		}
		
		// Return true if legs armor matches too.
		final int legMask = legs.getTemplate().getItemMask();
		return (_armorTypesMask & legMask) != 0;
	}
}
