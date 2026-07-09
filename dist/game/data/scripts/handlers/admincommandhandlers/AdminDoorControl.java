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

import java.awt.Color;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.logging.Logger;

import org.l2jmobius.gameserver.data.xml.DoorData;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.instancemanager.CastleManager;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Door;
import org.l2jmobius.gameserver.model.siege.Castle;
import org.l2jmobius.gameserver.network.serverpackets.ExServerPrimitive;
import org.l2jmobius.gameserver.util.BuilderUtil;

/**
 * This class handles following admin commands: - open1 = open coloseum door 24190001 - open2 = open coloseum door 24190002 - open3 = open coloseum door 24190003 - open4 = open coloseum door 24190004 - openall = open all coloseum door - close1 = close coloseum door 24190001 - close2 = close coloseum
 * door 24190002 - close3 = close coloseum door 24190003 - close4 = close coloseum door 24190004 - closeall = close all coloseum door - open = open selected door - close = close selected door
 * @version $Revision: 1.2.4.5 $ $Date: 2005/04/11 10:06:06 $
 */
public class AdminDoorControl implements IAdminCommandHandler
{
	private static final Logger LOGGER = Logger.getLogger(AdminDoorControl.class.getName());
	
	private static final Map<Player, Set<Integer>> PLAYER_SHOWN_DOORS = new ConcurrentHashMap<>();
	
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_open",
		"admin_close",
		"admin_openall",
		"admin_closeall",
		"admin_showdoors",
	};
	
	@Override
	public boolean useAdminCommand(String command, Player activeChar)
	{
		try
		{
			if (command.startsWith("admin_open "))
			{
				final int doorId = Integer.parseInt(command.substring(11));
				final Door door = DoorData.getInstance().getDoor(doorId);
				if (door != null)
				{
					door.openMe();
				}
				else
				{
					for (Castle castle : CastleManager.getInstance().getCastles())
					{
						if (castle.getDoor(doorId) != null)
						{
							castle.getDoor(doorId).openMe();
						}
					}
				}
			}
			else if (command.startsWith("admin_close "))
			{
				final int doorId = Integer.parseInt(command.substring(12));
				final Door door = DoorData.getInstance().getDoor(doorId);
				if (door != null)
				{
					door.closeMe();
				}
				else
				{
					for (Castle castle : CastleManager.getInstance().getCastles())
					{
						if (castle.getDoor(doorId) != null)
						{
							castle.getDoor(doorId).closeMe();
						}
					}
				}
			}
			else if (command.equals("admin_closeall"))
			{
				for (Door door : DoorData.getInstance().getDoors())
				{
					door.closeMe();
				}
				for (Castle castle : CastleManager.getInstance().getCastles())
				{
					for (Door door : castle.getDoors())
					{
						door.closeMe();
					}
				}
			}
			else if (command.equals("admin_openall"))
			{
				for (Door door : DoorData.getInstance().getDoors())
				{
					door.openMe();
				}
				for (Castle castle : CastleManager.getInstance().getCastles())
				{
					for (Door door : castle.getDoors())
					{
						door.openMe();
					}
				}
			}
			else if (command.equals("admin_open"))
			{
				final WorldObject target = activeChar.getTarget();
				if ((target != null) && target.isDoor())
				{
					((Door) target).openMe();
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "Incorrect target.");
				}
			}
			else if (command.equals("admin_close"))
			{
				final WorldObject target = activeChar.getTarget();
				if ((target != null) && target.isDoor())
				{
					((Door) target).closeMe();
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "Incorrect target.");
				}
			}
			else if (command.contains("admin_showdoors"))
			{
				if (command.contains("off"))
				{
					final Set<Integer> doorIds = PLAYER_SHOWN_DOORS.get(activeChar);
					if (doorIds == null)
					{
						return true;
					}
					
					for (int doorId : doorIds)
					{
						final ExServerPrimitive exsp = new ExServerPrimitive("Door" + doorId, activeChar.getX(), activeChar.getY(), -16000);
						exsp.addLine(Color.BLACK, activeChar.getX(), activeChar.getY(), -16000, activeChar.getX(), activeChar.getY(), -16000);
						activeChar.sendPacket(exsp);
					}
					
					doorIds.clear();
					PLAYER_SHOWN_DOORS.remove(activeChar);
				}
				else
				{
					final Set<Integer> doorIds;
					if (PLAYER_SHOWN_DOORS.containsKey(activeChar))
					{
						doorIds = PLAYER_SHOWN_DOORS.get(activeChar);
					}
					else
					{
						doorIds = new HashSet<>();
						PLAYER_SHOWN_DOORS.put(activeChar, doorIds);
					}
					
					World.getInstance().forEachVisibleObject(activeChar, Door.class, door ->
					{
						if (doorIds.contains(door.getId()))
						{
							return;
						}
						doorIds.add(door.getId());
						
						final ExServerPrimitive packet = new ExServerPrimitive("Door" + door.getId(), activeChar.getX(), activeChar.getY(), -16000);
						final Color color = door.isOpen() ? Color.GREEN : Color.RED;
						// box 1
						packet.addLine(color, door.getX(0), door.getY(0), door.getZMin(), door.getX(1), door.getY(1), door.getZMin());
						packet.addLine(color, door.getX(1), door.getY(1), door.getZMin(), door.getX(2), door.getY(2), door.getZMax());
						packet.addLine(color, door.getX(2), door.getY(2), door.getZMax(), door.getX(3), door.getY(3), door.getZMax());
						packet.addLine(color, door.getX(3), door.getY(3), door.getZMax(), door.getX(0), door.getY(0), door.getZMin());
						// box 2
						packet.addLine(color, door.getX(0), door.getY(0), door.getZMax(), door.getX(1), door.getY(1), door.getZMax());
						packet.addLine(color, door.getX(1), door.getY(1), door.getZMax(), door.getX(2), door.getY(2), door.getZMin());
						packet.addLine(color, door.getX(2), door.getY(2), door.getZMin(), door.getX(3), door.getY(3), door.getZMin());
						packet.addLine(color, door.getX(3), door.getY(3), door.getZMin(), door.getX(0), door.getY(0), door.getZMax());
						// diagonals
						packet.addLine(color, door.getX(0), door.getY(0), door.getZMin(), door.getX(1), door.getY(1), door.getZMax());
						packet.addLine(color, door.getX(2), door.getY(2), door.getZMin(), door.getX(3), door.getY(3), door.getZMax());
						packet.addLine(color, door.getX(0), door.getY(0), door.getZMax(), door.getX(1), door.getY(1), door.getZMin());
						packet.addLine(color, door.getX(2), door.getY(2), door.getZMax(), door.getX(3), door.getY(3), door.getZMin());
						activeChar.sendPacket(packet);
						// send message
						BuilderUtil.sendSysMessage(activeChar, "Found door " + door.getId());
					});
				}
			}
		}
		catch (Exception e)
		{
			LOGGER.warning("Problem with AdminDoorControl: " + e.getMessage());
		}
		return true;
	}
	
	@Override
	public String[] getAdminCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
