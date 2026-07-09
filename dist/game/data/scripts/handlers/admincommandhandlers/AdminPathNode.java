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

import java.util.List;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.geoengine.pathfinding.AbstractNodeLoc;
import org.l2jmobius.gameserver.geoengine.pathfinding.PathFinding;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.util.BuilderUtil;

public class AdminPathNode implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_path_find"
	};
	
	@Override
	public boolean useAdminCommand(String command, Player activeChar)
	{
		if (command.equals("admin_path_find"))
		{
			if (Config.PATHFINDING < 1)
			{
				BuilderUtil.sendSysMessage(activeChar, "PathFinding is disabled.");
				return true;
			}
			
			if (activeChar.getTarget() != null)
			{
				final List<AbstractNodeLoc> path = PathFinding.getInstance().findPath(activeChar.getX(), activeChar.getY(), (short) activeChar.getZ(), activeChar.getTarget().getX(), activeChar.getTarget().getY(), (short) activeChar.getTarget().getZ(), activeChar.getInstanceWorld(), true);
				if (path == null)
				{
					BuilderUtil.sendSysMessage(activeChar, "No Route!");
					return true;
				}
				for (AbstractNodeLoc a : path)
				{
					BuilderUtil.sendSysMessage(activeChar, "x:" + a.getX() + " y:" + a.getY() + " z:" + a.getZ());
				}
			}
			else
			{
				BuilderUtil.sendSysMessage(activeChar, "No Target!");
			}
		}
		return true;
	}
	
	@Override
	public String[] getAdminCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
