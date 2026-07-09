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
package handlers.bypasshandlers;

import org.l2jmobius.gameserver.data.sql.CharInfoTable;
import org.l2jmobius.gameserver.handler.IBypassHandler;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.itemcontainer.PlayerInventory;
import org.l2jmobius.gameserver.network.serverpackets.PartySmallWindowAll;
import org.l2jmobius.gameserver.network.serverpackets.PartySmallWindowDeleteAll;
import org.l2jmobius.gameserver.util.Util;

/**
 * @author Mobius
 */
public class ChangePlayerName implements IBypassHandler
{
	private static final int NAME_CHANGE_TICKET = 23622;
	
	private static final String[] COMMANDS =
	{
		"ChangePlayerName"
	};
	
	@Override
	public boolean useBypass(String command, Player player, Creature target)
	{
		// Need to have at least one Name Change Ticket in order to proceed.
		final PlayerInventory inventory = player.getInventory();
		if (inventory.getAllItemsByItemId(NAME_CHANGE_TICKET).isEmpty())
		{
			return false;
		}
		
		final String newName = command.split(" ")[1].trim();
		if (!Util.isAlphaNumeric(newName))
		{
			player.sendMessage("Name must only contain alphanumeric characters.");
			return false;
		}
		if (CharInfoTable.getInstance().doesCharNameExist(newName))
		{
			player.sendMessage("Name " + newName + " already exists.");
			return false;
		}
		
		// Destroy item.
		player.destroyItemByItemId("ChangePlayerName to " + newName, NAME_CHANGE_TICKET, 1, player, true);
		
		// Set name and proceed.
		player.setName(newName);
		CharInfoTable.getInstance().addName(player);
		player.storeMe();
		
		player.sendMessage("Your name has been changed.");
		player.broadcastUserInfo();
		
		if (player.isInParty())
		{
			// Delete party window for other party members
			player.getParty().broadcastToPartyMembers(player, PartySmallWindowDeleteAll.STATIC_PACKET);
			for (Player member : player.getParty().getMembers())
			{
				// And re-add
				if (member != player)
				{
					member.sendPacket(new PartySmallWindowAll(member, player.getParty()));
				}
			}
		}
		if (player.getClan() != null)
		{
			player.getClan().broadcastClanStatus();
		}
		
		return true;
	}
	
	@Override
	public String[] getBypassList()
	{
		return COMMANDS;
	}
}
