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
package handlers.chat.commands.admin;

import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.AttributeType;
import org.l2jmobius.gameserver.model.item.enchant.attribute.AttributeHolder;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.InventoryUpdate;

/**
 * This class handles following admin commands: - delete = deletes target
 * @version $Revision: 1.2.2.1.2.4 $ $Date: 2005/04/11 10:05:56 $
 */
public class AdminElement implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_setlh",
		"admin_setlc",
		"admin_setll",
		"admin_setlg",
		"admin_setlb",
		"admin_setlw",
		"admin_setls"
	};
	
	@Override
	public boolean onCommand(String command, Player activeChar)
	{
		int armorType = -1;
		if (command.startsWith("admin_setlh"))
		{
			armorType = Inventory.PAPERDOLL_HEAD;
		}
		else if (command.startsWith("admin_setlc"))
		{
			armorType = Inventory.PAPERDOLL_CHEST;
		}
		else if (command.startsWith("admin_setlg"))
		{
			armorType = Inventory.PAPERDOLL_GLOVES;
		}
		else if (command.startsWith("admin_setlb"))
		{
			armorType = Inventory.PAPERDOLL_FEET;
		}
		else if (command.startsWith("admin_setll"))
		{
			armorType = Inventory.PAPERDOLL_LEGS;
		}
		else if (command.startsWith("admin_setlw"))
		{
			armorType = Inventory.PAPERDOLL_RHAND;
		}
		else if (command.startsWith("admin_setls"))
		{
			armorType = Inventory.PAPERDOLL_LHAND;
		}
		
		if (armorType != -1)
		{
			try
			{
				final String[] args = command.split(" ");
				final AttributeType type = AttributeType.findByName(args[1]);
				final int value = Integer.parseInt(args[2]);
				if ((type == null) || (value < 0) || (value > 450))
				{
					activeChar.sendSysMessage("Usage: //setlh/setlc/setlg/setlb/setll/setlw/setls <element> <value>[0-450]");
					return false;
				}
				
				setElement(activeChar, type, value, armorType);
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //setlh/setlc/setlg/setlb/setll/setlw/setls <element>[0-5] <value>[0-450]");
				return false;
			}
		}
		
		return true;
	}
	
	@Override
	public String[] getCommandList()
	{
		return ADMIN_COMMANDS;
	}
	
	private void setElement(Player activeChar, AttributeType type, int value, int armorType)
	{
		// get the target
		WorldObject target = activeChar.getTarget();
		if (target == null)
		{
			target = activeChar;
		}
		
		Player player = null;
		if (target.isPlayer())
		{
			player = target.asPlayer();
		}
		else
		{
			activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
			return;
		}
		
		Item itemInstance = null;
		
		// only attempt to enchant if there is a weapon equipped
		final Item parmorInstance = player.getInventory().getPaperdollItem(armorType);
		if ((parmorInstance != null) && (parmorInstance.getLocationSlot() == armorType))
		{
			itemInstance = parmorInstance;
		}
		
		if (itemInstance != null)
		{
			String old;
			String current;
			final AttributeHolder element = itemInstance.getAttribute(type);
			if (element == null)
			{
				old = "None";
			}
			else
			{
				old = element.toString();
			}
			
			// set enchant value
			player.getInventory().unEquipItemInSlot(armorType);
			if (type == AttributeType.NONE)
			{
				itemInstance.clearAllAttributes();
			}
			else if (value < 1)
			{
				itemInstance.clearAttribute(type);
			}
			else
			{
				itemInstance.setAttribute(new AttributeHolder(type, value), true);
			}
			
			player.getInventory().equipItem(itemInstance);
			
			if (itemInstance.getAttributes() == null)
			{
				current = "None";
			}
			else
			{
				current = itemInstance.getAttribute(type).toString();
			}
			
			// send packets
			final InventoryUpdate iu = new InventoryUpdate();
			iu.addModifiedItem(itemInstance);
			player.sendInventoryUpdate(iu);
			
			// informations
			activeChar.sendSysMessage("Changed elemental power of " + player.getName() + "'s " + itemInstance.getTemplate().getName() + " from " + old + " to " + current + ".");
			if (player != activeChar)
			{
				player.sendMessage(activeChar.getName() + " has changed the elemental power of your " + itemInstance.getTemplate().getName() + " from " + old + " to " + current + ".");
			}
		}
	}
}
