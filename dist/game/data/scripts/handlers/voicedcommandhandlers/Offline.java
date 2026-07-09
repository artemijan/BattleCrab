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
package handlers.voicedcommandhandlers;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.handler.IVoicedCommandHandler;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ActionFailed;
import org.l2jmobius.gameserver.network.serverpackets.ConfirmDlg;

/**
 * @author Mobius
 */
public class Offline implements IVoicedCommandHandler
{
	private static final String[] VOICED_COMMANDS =
	{
		"offline"
	};
	
	@Override
	public boolean useVoicedCommand(String command, Player player, String target)
	{
		if (command.equals("offline") && Config.ENABLE_OFFLINE_COMMAND && (Config.OFFLINE_TRADE_ENABLE || Config.OFFLINE_CRAFT_ENABLE))
		{
			if (!player.isInStoreMode())
			{
				player.sendPacket(SystemMessageId.PRIVATE_STORE_ALREADY_CLOSED);
				player.sendPacket(ActionFailed.STATIC_PACKET);
				return false;
			}
			
			if (player.isInInstance() || player.isInVehicle() || !player.canLogout())
			{
				player.sendPacket(ActionFailed.STATIC_PACKET);
				return false;
			}
			
			player.sendPacket(new ConfirmDlg(SystemMessageId.DO_YOU_WISH_TO_EXIT_THE_GAME));
		}
		
		return true;
	}
	
	@Override
	public String[] getVoicedCommandList()
	{
		return VOICED_COMMANDS;
	}
}