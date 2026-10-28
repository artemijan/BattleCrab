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

import java.util.EnumSet;
import java.util.List;
import java.util.Set;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.item.enums.BodyPart;
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
		
		final Creature targetCreature = target.asCreature();
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
		final BodyPart chestBodyPart = chest.getTemplate().getBodyPart();
		
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
			if (chestBodyPart == BodyPart.FULL_ARMOR)
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
