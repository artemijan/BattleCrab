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

import java.util.Collections;
import java.util.List;

import org.l2jmobius.gameserver.data.SpawnTable;
import org.l2jmobius.gameserver.handler.AdminCommandHandler;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.instancemanager.DBSpawnManager;
import org.l2jmobius.gameserver.model.Spawn;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.spawns.NpcSpawnTemplate;
import org.l2jmobius.gameserver.model.spawns.SpawnGroup;
import org.l2jmobius.gameserver.model.spawns.SpawnTemplate;
import org.l2jmobius.gameserver.model.zone.type.SpawnTerritory;
import org.l2jmobius.gameserver.util.BuilderUtil;
import org.l2jmobius.gameserver.util.Util;

/**
 * @author Mobius
 */
public class AdminDelete implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_delete", // supports range parameter
		"admin_delete_group" // for territory spawns
	};
	
	@Override
	public boolean useAdminCommand(String command, Player activeChar)
	{
		if (command.contains("group"))
		{
			handleDeleteGroup(activeChar);
		}
		else if (command.startsWith("admin_delete"))
		{
			final String[] split = command.split(" ");
			handleDelete(activeChar, (split.length > 1) && Util.isDigit(split[1]) ? Integer.parseInt(split[1]) : 0);
		}
		return true;
	}
	
	private void handleDelete(Player player, int range)
	{
		if (range > 0)
		{
			World.getInstance().forEachVisibleObjectInRange(player, Npc.class, range, target -> deleteNpc(player, target));
			return;
		}
		
		final WorldObject obj = player.getTarget();
		if (obj instanceof Npc)
		{
			deleteNpc(player, (Npc) obj);
		}
		else
		{
			BuilderUtil.sendSysMessage(player, "Incorrect target.");
		}
	}
	
	private void handleDeleteGroup(Player player)
	{
		final WorldObject obj = player.getTarget();
		if (obj instanceof Npc)
		{
			deleteGroup(player, (Npc) obj);
		}
		else
		{
			BuilderUtil.sendSysMessage(player, "Incorrect target.");
		}
	}
	
	private void deleteNpc(Player player, Npc target)
	{
		final Spawn spawn = target.getSpawn();
		if (spawn != null)
		{
			final NpcSpawnTemplate npcSpawnTemplate = spawn.getNpcSpawnTemplate();
			final SpawnGroup group = npcSpawnTemplate != null ? npcSpawnTemplate.getGroup() : null;
			List<SpawnTerritory> territories = group != null ? group.getTerritories() : Collections.emptyList();
			if (territories.isEmpty())
			{
				final SpawnTemplate spawnTemplate = npcSpawnTemplate != null ? npcSpawnTemplate.getSpawnTemplate() : null;
				if (spawnTemplate != null)
				{
					territories = spawnTemplate.getTerritories();
				}
			}
			if (territories.isEmpty())
			{
				target.deleteMe();
				spawn.stopRespawn();
				if (DBSpawnManager.getInstance().isDefined(spawn.getId()))
				{
					DBSpawnManager.getInstance().deleteSpawn(spawn, true);
				}
				else
				{
					SpawnTable.getInstance().deleteSpawn(spawn, true);
				}
				BuilderUtil.sendSysMessage(player, "Deleted " + target.getName() + " from " + target.getObjectId() + ".");
			}
			else
			{
				AdminCommandHandler.getInstance().useAdminCommand(player, AdminDelete.ADMIN_COMMANDS[1], true);
			}
		}
	}
	
	private void deleteGroup(Player player, Npc target)
	{
		final Spawn spawn = target.getSpawn();
		if (spawn != null)
		{
			final NpcSpawnTemplate npcSpawnTemplate = spawn.getNpcSpawnTemplate();
			final SpawnGroup group = npcSpawnTemplate != null ? npcSpawnTemplate.getGroup() : null;
			List<SpawnTerritory> territories = group != null ? group.getTerritories() : Collections.emptyList();
			boolean simpleTerritory = false;
			if (territories.isEmpty())
			{
				final SpawnTemplate spawnTemplate = npcSpawnTemplate != null ? npcSpawnTemplate.getSpawnTemplate() : null;
				if (spawnTemplate != null)
				{
					territories = spawnTemplate.getTerritories();
					simpleTerritory = true;
				}
			}
			if (territories.isEmpty())
			{
				BuilderUtil.sendSysMessage(player, "Incorrect target.");
			}
			else
			{
				target.deleteMe();
				spawn.stopRespawn();
				if (DBSpawnManager.getInstance().isDefined(spawn.getId()))
				{
					DBSpawnManager.getInstance().deleteSpawn(spawn, true);
				}
				else
				{
					SpawnTable.getInstance().deleteSpawn(spawn, true);
				}
				
				if (group != null)
				{
					for (NpcSpawnTemplate template : group.getSpawns())
					{
						template.despawn();
					}
				}
				else if (simpleTerritory && (npcSpawnTemplate != null))
				{
					npcSpawnTemplate.despawn();
				}
				
				BuilderUtil.sendSysMessage(player, "Deleted " + target.getName() + " group from " + target.getObjectId() + ".");
			}
		}
	}
	
	@Override
	public String[] getAdminCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
