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

import java.util.List;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.Weapon;
import org.l2jmobius.gameserver.model.item.enchant.attribute.AttributeHolder;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.InventoryUpdate;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * Convert Item effect implementation.
 * @author Zoey76
 */
public class ConvertItem extends AbstractEffect
{
	public ConvertItem(StatSet params)
	{
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effected.isAlikeDead() || !effected.isPlayer())
		{
			return;
		}
		
		final Player player = effected.getActingPlayer();
		if (player.hasItemRequest())
		{
			return;
		}
		
		final Weapon weaponItem = player.getActiveWeaponItem();
		if (weaponItem == null)
		{
			return;
		}
		
		Item wpn = player.getInventory().getPaperdollItem(Inventory.PAPERDOLL_RHAND);
		if (wpn == null)
		{
			wpn = player.getInventory().getPaperdollItem(Inventory.PAPERDOLL_LHAND);
		}
		
		if ((wpn == null) || wpn.isAugmented() || (weaponItem.getChangeWeaponId() == 0))
		{
			return;
		}
		
		final int newItemId = weaponItem.getChangeWeaponId();
		if (newItemId == -1)
		{
			return;
		}
		
		final int enchantLevel = wpn.getEnchantLevel();
		final AttributeHolder elementals = wpn.getAttributes() == null ? null : wpn.getAttackAttribute();
		final List<Item> unequipped = player.getInventory().unEquipItemInBodySlotAndRecord(wpn.getTemplate().getBodyPart());
		final InventoryUpdate iu = new InventoryUpdate();
		for (Item unequippedItem : unequipped)
		{
			iu.addModifiedItem(unequippedItem);
		}
		player.sendInventoryUpdate(iu);
		
		if (unequipped.isEmpty())
		{
			return;
		}
		
		byte count = 0;
		for (Item unequippedItem : unequipped)
		{
			if (!(unequippedItem.getTemplate() instanceof Weapon))
			{
				count++;
				continue;
			}
			
			final SystemMessage sm;
			if (unequippedItem.getEnchantLevel() > 0)
			{
				sm = new SystemMessage(SystemMessageId.THE_EQUIPMENT_S1_S2_HAS_BEEN_REMOVED);
				sm.addInt(unequippedItem.getEnchantLevel());
				sm.addItemName(unequippedItem);
			}
			else
			{
				sm = new SystemMessage(SystemMessageId.S1_HAS_BEEN_UNEQUIPPED);
				sm.addItemName(unequippedItem);
			}
			player.sendPacket(sm);
		}
		
		if (count == unequipped.size())
		{
			return;
		}
		
		final Item destroyItem = player.getInventory().destroyItem("ChangeWeapon", wpn, player, null);
		if (destroyItem == null)
		{
			return;
		}
		
		final Item newItem = player.getInventory().addItem("ChangeWeapon", newItemId, 1, player, destroyItem);
		if (newItem == null)
		{
			return;
		}
		
		if (elementals != null)
		{
			newItem.setAttribute(elementals, true);
		}
		newItem.setEnchantLevel(enchantLevel);
		player.getInventory().equipItem(newItem);
		
		final SystemMessage msg;
		if (newItem.getEnchantLevel() > 0)
		{
			msg = new SystemMessage(SystemMessageId.EQUIPPED_S1_S2);
			msg.addInt(newItem.getEnchantLevel());
			msg.addItemName(newItem);
		}
		else
		{
			msg = new SystemMessage(SystemMessageId.YOU_HAVE_EQUIPPED_YOUR_S1);
			msg.addItemName(newItem);
		}
		player.sendPacket(msg);
		
		final InventoryUpdate u = new InventoryUpdate();
		u.addRemovedItem(destroyItem);
		u.addItem(newItem);
		player.sendInventoryUpdate(u);
		
		player.broadcastUserInfo();
	}
}
