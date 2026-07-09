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

import org.l2jmobius.gameserver.handler.IVoicedCommandHandler;
import org.l2jmobius.gameserver.model.actor.Player;

/**
 * @author xban1x
 */
public class ExperienceGain implements IVoicedCommandHandler
{
	private static final String[] COMMANDS = new String[]
	{
		"expoff",
		"expon",
	};
	
	@Override
	public boolean useVoicedCommand(String command, Player player, String params)
	{
		if (command.equals("expoff"))
		{
			if (!player.getVariables().getBoolean("EXPOFF", false))
			{
				player.disableExpGain();
				player.getVariables().set("EXPOFF", true);
				player.sendMessage("Experience gain is disabled.");
			}
		}
		else if (command.equals("expon"))
		{
			if (player.getVariables().getBoolean("EXPOFF", false))
			{
				player.enableExpGain();
				player.getVariables().set("EXPOFF", false);
				player.sendMessage("Experience gain is enabled.");
			}
		}
		return true;
	}
	
	@Override
	public String[] getVoicedCommandList()
	{
		return COMMANDS;
	}
}
