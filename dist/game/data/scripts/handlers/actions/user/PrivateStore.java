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
package handlers.actions.user;

import java.util.logging.Logger;

import org.l2jmobius.gameserver.data.holders.ActionDataHolder;
import org.l2jmobius.gameserver.handler.IActionUserHandler;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.player.PrivateStoreType;
import org.l2jmobius.gameserver.model.zone.ZoneId;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ActionFailed;
import org.l2jmobius.gameserver.network.serverpackets.PrivateStoreManageListBuy;
import org.l2jmobius.gameserver.network.serverpackets.PrivateStoreManageListSell;
import org.l2jmobius.gameserver.network.serverpackets.RecipeShopManageList;

/**
 * Open/Close private store player action handler.
 * @author Nik
 */
public class PrivateStore implements IActionUserHandler
{
	private static final Logger LOGGER = Logger.getLogger(PrivateStore.class.getName());
	
	@Override
	public void onAction(Player player, ActionDataHolder data, boolean ctrlPressed, boolean shiftPressed)
	{
		final PrivateStoreType type = PrivateStoreType.findById(data.getOptionId());
		if (type == null)
		{
			LOGGER.warning("Incorrect private store type: " + data.getOptionId());
			return;
		}
		
		// Player shouldn't be able to set stores if he/she is alike dead (dead or fake death)
		if (!player.canOpenPrivateStore())
		{
			if (player.isInsideZone(ZoneId.NO_STORE))
			{
				player.sendPacket(SystemMessageId.YOU_CANNOT_OPEN_A_PRIVATE_STORE_HERE);
			}
			
			player.sendPacket(ActionFailed.STATIC_PACKET);
			return;
		}
		
		switch (type)
		{
			case SELL:
			case SELL_MANAGE:
			case PACKAGE_SELL:
			{
				if ((player.getPrivateStoreType() == PrivateStoreType.SELL) || (player.getPrivateStoreType() == PrivateStoreType.SELL_MANAGE) || (player.getPrivateStoreType() == PrivateStoreType.PACKAGE_SELL))
				{
					player.setPrivateStoreType(PrivateStoreType.NONE);
					player.getSellList().clear();
				}
				break;
			}
			case BUY:
			case BUY_MANAGE:
			{
				if ((player.getPrivateStoreType() == PrivateStoreType.BUY) || (player.getPrivateStoreType() == PrivateStoreType.BUY_MANAGE))
				{
					player.setPrivateStoreType(PrivateStoreType.NONE);
				}
				break;
			}
			case MANUFACTURE:
			{
				player.setPrivateStoreType(PrivateStoreType.NONE);
				player.broadcastUserInfo();
			}
		}
		
		if (player.getPrivateStoreType() == PrivateStoreType.NONE)
		{
			if (player.isSitting())
			{
				player.standUp();
			}
			
			switch (type)
			{
				case SELL:
				case SELL_MANAGE:
				case PACKAGE_SELL:
				{
					player.setPrivateStoreType(PrivateStoreType.SELL_MANAGE);
					player.sendPacket(new PrivateStoreManageListSell(1, player, type == PrivateStoreType.PACKAGE_SELL));
					player.sendPacket(new PrivateStoreManageListSell(2, player, type == PrivateStoreType.PACKAGE_SELL));
					break;
				}
				case BUY:
				case BUY_MANAGE:
				{
					player.setPrivateStoreType(PrivateStoreType.BUY_MANAGE);
					player.sendPacket(new PrivateStoreManageListBuy(1, player));
					player.sendPacket(new PrivateStoreManageListBuy(2, player));
					break;
				}
				case MANUFACTURE:
				{
					player.sendPacket(new RecipeShopManageList(player, true));
				}
			}
		}
	}
}
