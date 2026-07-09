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
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.item.type.ArmorType;
import org.l2jmobius.gameserver.model.item.type.ItemType;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.skill.ISkillCondition;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Mobius
 */
public class OpTargetArmorTypeSkillCondition implements ISkillCondition
{
	private final Set<ArmorType> _armorTypes = EnumSet.noneOf(ArmorType.class);
	
	public OpTargetArmorTypeSkillCondition(StatSet params)
	{
		final List<String> armorTypes = params.getList("armorType", String.class);
		if (armorTypes != null)
		{
			for (String type : armorTypes)
			{
				_armorTypes.add(ArmorType.valueOf(type));
			}
		}
	}
	
	@Override
	public boolean canUse(Creature caster, Skill skill, WorldObject target)
	{
		if ((target == null) || !target.isCreature())
		{
			return false;
		}
		
		final Creature targetCreature = (Creature) target;
		final Inventory inv = targetCreature.getInventory();
		
		// Get the chest armor.
		final Item chest = inv.getPaperdollItem(Inventory.PAPERDOLL_CHEST);
		if (chest == null)
		{
			return false;
		}
		
		// Get the chest item type.
		final ItemType chestType = chest.getTemplate().getItemType();
		
		// Get the chest body part.
		final long chestBodyPart = chest.getTemplate().getBodyPart();
		
		// Get the legs armor.
		final Item legs = inv.getPaperdollItem(Inventory.PAPERDOLL_LEGS);
		
		// Get the legs item type.
		ItemType legsType = null;
		if (legs != null)
		{
			legsType = legs.getTemplate().getItemType();
		}
		
		for (ArmorType armorType : _armorTypes)
		{
			// If chest armor is different from the condition one continue.
			if (chestType != armorType)
			{
				continue;
			}
			
			// Return true if chest armor is a full armor.
			if (chestBodyPart == ItemTemplate.SLOT_FULL_ARMOR)
			{
				return true;
			}
			
			// Check legs armor.
			if (legs == null)
			{
				continue;
			}
			
			// Return true if legs armor matches too.
			if (legsType == armorType)
			{
				return true;
			}
		}
		
		return false;
	}
}
