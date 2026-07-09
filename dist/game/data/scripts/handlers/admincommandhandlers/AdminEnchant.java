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
package handlers.admincommandhandlers;

import java.util.logging.Logger;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.data.xml.EnchantItemGroupsData;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.ItemTemplate;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.InventoryUpdate;
import org.l2jmobius.gameserver.util.BuilderUtil;

/**
 * This class handles following admin commands: - enchant_armor
 * @version $Revision: 1.3.2.1.2.10 $ $Date: 2005/08/24 21:06:06 $
 */
public class AdminEnchant implements IAdminCommandHandler
{
	private static final Logger LOGGER = Logger.getLogger(AdminEnchant.class.getName());
	
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_seteh", // 6
		"admin_setec", // 10
		"admin_seteg", // 9
		"admin_setel", // 11
		"admin_seteb", // 12
		"admin_setew", // 7
		"admin_setes", // 8
		"admin_setle", // 1
		"admin_setre", // 2
		"admin_setlf", // 4
		"admin_setrf", // 5
		"admin_seten", // 3
		"admin_setun", // 0
		"admin_setba", // 13
		"admin_setbe",
		"admin_enchant"
	};
	
	@Override
	public boolean useAdminCommand(String command, Player activeChar)
	{
		if (command.equals("admin_enchant"))
		{
			showMainPage(activeChar);
		}
		else
		{
			int slot = -1;
			if (command.startsWith("admin_seteh"))
			{
				slot = Inventory.PAPERDOLL_HEAD;
			}
			else if (command.startsWith("admin_setec"))
			{
				slot = Inventory.PAPERDOLL_CHEST;
			}
			else if (command.startsWith("admin_seteg"))
			{
				slot = Inventory.PAPERDOLL_GLOVES;
			}
			else if (command.startsWith("admin_seteb"))
			{
				slot = Inventory.PAPERDOLL_FEET;
			}
			else if (command.startsWith("admin_setel"))
			{
				slot = Inventory.PAPERDOLL_LEGS;
			}
			else if (command.startsWith("admin_setew"))
			{
				slot = Inventory.PAPERDOLL_RHAND;
			}
			else if (command.startsWith("admin_setes"))
			{
				slot = Inventory.PAPERDOLL_LHAND;
			}
			else if (command.startsWith("admin_setle"))
			{
				slot = Inventory.PAPERDOLL_LEAR;
			}
			else if (command.startsWith("admin_setre"))
			{
				slot = Inventory.PAPERDOLL_REAR;
			}
			else if (command.startsWith("admin_setlf"))
			{
				slot = Inventory.PAPERDOLL_LFINGER;
			}
			else if (command.startsWith("admin_setrf"))
			{
				slot = Inventory.PAPERDOLL_RFINGER;
			}
			else if (command.startsWith("admin_seten"))
			{
				slot = Inventory.PAPERDOLL_NECK;
			}
			else if (command.startsWith("admin_setun"))
			{
				slot = Inventory.PAPERDOLL_UNDER;
			}
			else if (command.startsWith("admin_setba"))
			{
				slot = Inventory.PAPERDOLL_CLOAK;
			}
			else if (command.startsWith("admin_setbe"))
			{
				slot = Inventory.PAPERDOLL_BELT;
			}
			
			if (slot != -1)
			{
				try
				{
					final int ench = Integer.parseInt(command.substring(12));
					
					// check value
					if ((ench < 0) || (ench > 127))
					{
						BuilderUtil.sendSysMessage(activeChar, "You must set the enchant level to be between 0-127.");
					}
					else
					{
						setEnchant(activeChar, ench, slot);
					}
				}
				catch (StringIndexOutOfBoundsException e)
				{
					if (Config.DEVELOPER)
					{
						LOGGER.warning("Set enchant error: " + e);
					}
					BuilderUtil.sendSysMessage(activeChar, "Please specify a new enchant value.");
				}
				catch (NumberFormatException e)
				{
					if (Config.DEVELOPER)
					{
						LOGGER.warning("Set enchant error: " + e);
					}
					BuilderUtil.sendSysMessage(activeChar, "Please specify a valid new enchant value.");
				}
			}
			
			// show the enchant menu after an action
			showMainPage(activeChar);
		}
		return true;
	}
	
	private void setEnchant(Player activeChar, int ench, int slot)
	{
		// Get the target.
		final Player player = activeChar.getTarget() != null ? activeChar.getTarget().getActingPlayer() : activeChar;
		if (player == null)
		{
			activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
			return;
		}
		
		// Now we need to find the equipped weapon of the targeted character...
		Item itemInstance = null;
		
		// Only attempt to enchant if there is a weapon equipped.
		final Item paperdollInstance = player.getInventory().getPaperdollItem(slot);
		if ((paperdollInstance != null) && (paperdollInstance.getLocationSlot() == slot))
		{
			itemInstance = paperdollInstance;
		}
		
		if (itemInstance != null)
		{
			final int curEnchant = itemInstance.getEnchantLevel();
			
			// Set enchant value.
			int enchant = ench;
			if (Config.OVER_ENCHANT_PROTECTION && !player.isGM())
			{
				if (itemInstance.isWeapon())
				{
					if (enchant > EnchantItemGroupsData.getInstance().getMaxWeaponEnchant())
					{
						BuilderUtil.sendSysMessage(activeChar, "Maximum enchantment for weapon items is " + EnchantItemGroupsData.getInstance().getMaxWeaponEnchant() + ".");
						enchant = EnchantItemGroupsData.getInstance().getMaxWeaponEnchant();
					}
				}
				else if (itemInstance.getTemplate().getType2() == ItemTemplate.TYPE2_ACCESSORY)
				{
					if (enchant > EnchantItemGroupsData.getInstance().getMaxAccessoryEnchant())
					{
						BuilderUtil.sendSysMessage(activeChar, "Maximum enchantment for accessory items is " + EnchantItemGroupsData.getInstance().getMaxAccessoryEnchant() + ".");
						enchant = EnchantItemGroupsData.getInstance().getMaxAccessoryEnchant();
					}
				}
				else if (enchant > EnchantItemGroupsData.getInstance().getMaxArmorEnchant())
				{
					BuilderUtil.sendSysMessage(activeChar, "Maximum enchantment for armor items is " + EnchantItemGroupsData.getInstance().getMaxArmorEnchant() + ".");
					enchant = EnchantItemGroupsData.getInstance().getMaxArmorEnchant();
				}
			}
			player.getInventory().unEquipItemInSlot(slot);
			itemInstance.setEnchantLevel(enchant);
			player.getInventory().equipItem(itemInstance);
			
			// Send packets.
			final InventoryUpdate iu = new InventoryUpdate();
			iu.addModifiedItem(itemInstance);
			player.sendInventoryUpdate(iu);
			player.broadcastUserInfo();
			
			// Information.
			BuilderUtil.sendSysMessage(activeChar, "Changed enchantment of " + player.getName() + "'s " + itemInstance.getTemplate().getName() + " from " + curEnchant + " to " + enchant + ".");
			player.sendMessage("Admin has changed the enchantment of your " + itemInstance.getTemplate().getName() + " from " + curEnchant + " to " + enchant + ".");
		}
	}
	
	private void showMainPage(Player activeChar)
	{
		AdminHtml.showAdminHtml(activeChar, "enchant.htm");
	}
	
	@Override
	public String[] getAdminCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
