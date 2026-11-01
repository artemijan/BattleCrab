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
package handlers.bypass.npc;

import java.util.logging.Level;

import org.l2jmobius.gameserver.config.GeneralConfig;
import org.l2jmobius.gameserver.handler.IBypassHandler;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Warehouse;
import org.l2jmobius.gameserver.model.clan.Clan;
import org.l2jmobius.gameserver.model.clan.ClanAccess;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ActionFailed;
import org.l2jmobius.gameserver.network.serverpackets.WareHouseDepositList;
import org.l2jmobius.gameserver.network.serverpackets.WareHouseWithdrawalList;

public class ClanWarehouse implements IBypassHandler
{
	private static final String[] COMMANDS =
	{
		"withdrawc",
		"depositc"
	};
	
	@Override
	public boolean onCommand(String command, Player player, Creature target)
	{
		if (!GeneralConfig.ALLOW_WAREHOUSE)
		{
			return false;
		}
		
		if (!target.isNpc())
		{
			return false;
		}
		
		final Npc npc = target.asNpc();
		if (!(npc instanceof Warehouse) && (npc.getClan() != null))
		{
			return false;
		}
		
		if (player.hasItemRequest())
		{
			return false;
		}
		
		final Clan clan = player.getClan();
		if (clan == null)
		{
			player.sendPacket(SystemMessageId.YOU_DO_NOT_HAVE_THE_RIGHT_TO_USE_THE_CLAN_WAREHOUSE);
			return false;
		}
		else if (clan.getLevel() == 0)
		{
			player.sendPacket(SystemMessageId.ONLY_CLANS_OF_CLAN_LEVEL_1_OR_ABOVE_CAN_USE_A_CLAN_WAREHOUSE);
			return false;
		}
		else
		{
			try
			{
				if (command.toLowerCase().startsWith(COMMANDS[0])) // WithdrawC
				{
					player.sendPacket(ActionFailed.STATIC_PACKET);
					
					if (!player.hasAccess(ClanAccess.ACCESS_WAREHOUSE))
					{
						player.sendPacket(SystemMessageId.YOU_DO_NOT_HAVE_THE_RIGHT_TO_USE_THE_CLAN_WAREHOUSE);
						return true;
					}
					
					player.setActiveWarehouse(clan.getWarehouse());
					
					if (player.getActiveWarehouse().getSize() == 0)
					{
						player.sendPacket(SystemMessageId.YOU_HAVE_NOT_DEPOSITED_ANY_ITEMS_IN_YOUR_WAREHOUSE);
						return true;
					}
					
					for (Item i : player.getActiveWarehouse().getItems())
					{
						if (i.isTimeLimitedItem() && (i.getRemainingTime() <= 0))
						{
							player.getActiveWarehouse().destroyItem(ItemProcessType.DESTROY, i, player, null);
						}
					}
					
					player.sendPacket(new WareHouseWithdrawalList(1, player, WareHouseWithdrawalList.CLAN));
					player.sendPacket(new WareHouseWithdrawalList(2, player, WareHouseWithdrawalList.CLAN));
					return true;
				}
				else if (command.toLowerCase().startsWith(COMMANDS[1])) // DepositC
				{
					player.sendPacket(ActionFailed.STATIC_PACKET);
					player.setActiveWarehouse(clan.getWarehouse());
					player.setInventoryBlockingStatus(true);
					player.sendPacket(new WareHouseDepositList(1, player, WareHouseDepositList.CLAN));
					player.sendPacket(new WareHouseDepositList(2, player, WareHouseDepositList.CLAN));
					return true;
				}
				
				return false;
			}
			catch (Exception e)
			{
				LOGGER.log(Level.WARNING, "Exception in " + getClass().getSimpleName(), e);
			}
		}
		
		return false;
	}
	
	@Override
	public String[] getCommandList()
	{
		return COMMANDS;
	}
}
