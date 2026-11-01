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
package handlers.items;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.data.xml.CategoryData;
import org.l2jmobius.gameserver.handler.IItemHandler;
import org.l2jmobius.gameserver.model.actor.Playable;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.InventoryUpdate;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * @author Mobius
 */
public class FatedSupportBox implements IItemHandler
{
	// Items
	private static final int FATED_BOX_ERTHEIA_WIZARD = 26229;
	private static final int FATED_BOX_ERTHEIA_FIGHTER = 26230;
	private static final int FATED_BOX_FIGHTER = 37315;
	private static final int FATED_BOX_WIZARD = 37316;
	private static final int FATED_BOX_WARRIOR = 37317;
	private static final int FATED_BOX_ROGUE = 37318;
	private static final int FATED_BOX_KAMAEL = 37319;
	private static final int FATED_BOX_ORC_FIGHTER = 37320;
	private static final int FATED_BOX_ORC_WIZARD = 37321;
	
	@Override
	public boolean onItemUse(Playable playable, Item item, boolean forceUse)
	{
		if (!playable.isPlayer())
		{
			playable.sendPacket(SystemMessageId.YOUR_PET_CANNOT_CARRY_THIS_ITEM);
			return false;
		}
		
		final Player player = playable.asPlayer();
		final Race race = player.getRace();
		final PlayerClass playerClass = player.getPlayerClass();
		
		if (!player.isInventoryUnder80(false))
		{
			player.sendPacket(SystemMessageId.YOU_VE_EXCEEDED_THE_LIMIT_AND_CANNOT_RETRIEVE_THE_ITEM_PLEASE_CHECK_YOUR_LIMIT_IN_THE_INVENTORY);
			return false;
		}
		
		// Characters that have gone through their 2nd class transfer/1st liberation will be able to open the Fated Support Box at level 40.
		if ((player.getLevel() < 40) || player.isInCategory(CategoryType.FIRST_CLASS_GROUP) || ((race != Race.ERTHEIA) && player.isInCategory(CategoryType.SECOND_CLASS_GROUP)))
		{
			player.sendPacket(new SystemMessage(SystemMessageId.S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS).addItemName(item));
			return false;
		}
		
		player.getInventory().destroyItem(ItemProcessType.FEE, item, 1, player, null);
		player.sendInventoryUpdate(new InventoryUpdate(item));
		
		// It will stay in your inventory after use until you reach level 84.
		if (player.getLevel() > 84)
		{
			player.sendMessage("Fated Support Box was removed because your level has exceeded the maximum requirement."); // custom message
			return true;
		}
		
		switch (race)
		{
			case HUMAN:
			case ELF:
			case DARK_ELF:
			case DWARF:
			{
				if (player.isMageClass())
				{
					player.addItem(ItemProcessType.REWARD, FATED_BOX_WIZARD, 1, player, true);
				}
				else if (CategoryData.getInstance().isInCategory(CategoryType.SUB_GROUP_ROGUE, playerClass.getId()))
				{
					player.addItem(ItemProcessType.REWARD, FATED_BOX_ROGUE, 1, player, true);
				}
				else if (CategoryData.getInstance().isInCategory(CategoryType.SUB_GROUP_KNIGHT, playerClass.getId()))
				{
					player.addItem(ItemProcessType.REWARD, FATED_BOX_FIGHTER, 1, player, true);
				}
				else
				{
					player.addItem(ItemProcessType.REWARD, FATED_BOX_WARRIOR, 1, player, true);
				}
				break;
			}
			case ORC:
			{
				if (player.isMageClass())
				{
					player.addItem(ItemProcessType.REWARD, FATED_BOX_ORC_WIZARD, 1, player, true);
				}
				else
				{
					player.addItem(ItemProcessType.REWARD, FATED_BOX_ORC_FIGHTER, 1, player, true);
				}
				break;
			}
			case KAMAEL:
			{
				player.addItem(ItemProcessType.REWARD, FATED_BOX_KAMAEL, 1, player, true);
				break;
			}
			case ERTHEIA:
			{
				if (player.isMageClass())
				{
					player.addItem(ItemProcessType.REWARD, FATED_BOX_ERTHEIA_WIZARD, 1, player, true);
				}
				else
				{
					player.addItem(ItemProcessType.REWARD, FATED_BOX_ERTHEIA_FIGHTER, 1, player, true);
				}
				break;
			}
		}
		
		return true;
	}
}
