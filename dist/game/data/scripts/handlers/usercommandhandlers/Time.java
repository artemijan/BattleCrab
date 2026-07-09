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
package handlers.usercommandhandlers;

import java.text.SimpleDateFormat;
import java.util.Date;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.handler.IUserCommandHandler;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;
import org.l2jmobius.gameserver.taskmanager.GameTimeTaskManager;

/**
 * Time user command.
 */
public class Time implements IUserCommandHandler
{
	private static final int[] COMMAND_IDS =
	{
		77
	};
	
	private static final SimpleDateFormat SDF = new SimpleDateFormat("H:mm.");
	
	@Override
	public boolean useUserCommand(int id, Player player)
	{
		if (COMMAND_IDS[0] != id)
		{
			return false;
		}
		
		final int t = GameTimeTaskManager.getInstance().getGameTime();
		final String h = Integer.toString(((t / 60) % 24));
		String m;
		if ((t % 60) < 10)
		{
			m = "0" + (t % 60);
		}
		else
		{
			m = Integer.toString((t % 60));
		}
		
		SystemMessage sm;
		if (GameTimeTaskManager.getInstance().isNight())
		{
			sm = new SystemMessage(SystemMessageId.THE_CURRENT_TIME_IS_S1_S2_2);
			sm.addString(h);
			sm.addString(m);
		}
		else
		{
			sm = new SystemMessage(SystemMessageId.THE_CURRENT_TIME_IS_S1_S2);
			sm.addString(h);
			sm.addString(m);
		}
		player.sendPacket(sm);
		if (Config.DISPLAY_SERVER_TIME)
		{
			player.sendMessage("Server time is " + SDF.format(new Date(System.currentTimeMillis())));
		}
		return true;
	}
	
	@Override
	public int[] getUserCommandList()
	{
		return COMMAND_IDS;
	}
}
