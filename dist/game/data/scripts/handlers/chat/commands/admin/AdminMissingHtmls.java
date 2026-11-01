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
package handlers.chat.commands.admin;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.StringTokenizer;

import org.l2jmobius.gameserver.cache.HtmCache;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.BroadcastingTower;
import org.l2jmobius.gameserver.model.actor.instance.Fisherman;
import org.l2jmobius.gameserver.model.actor.instance.FlyTerrainObject;
import org.l2jmobius.gameserver.model.actor.instance.Guard;
import org.l2jmobius.gameserver.model.actor.instance.Merchant;
import org.l2jmobius.gameserver.model.actor.instance.Warehouse;
import org.l2jmobius.gameserver.model.events.EventType;

/**
 * @author Mobius
 */
public class AdminMissingHtmls implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_geomap_missing_htmls",
		"admin_world_missing_htmls",
		"admin_next_missing_html"
	};
	
	@Override
	public boolean onCommand(String command, Player activeChar)
	{
		final StringTokenizer st = new StringTokenizer(command, " ");
		final String actualCommand = st.nextToken();
		switch (actualCommand.toLowerCase())
		{
			case "admin_geomap_missing_htmls":
			{
				final int x = ((activeChar.getX() - World.WORLD_X_MIN) >> 15) + World.TILE_X_MIN;
				final int y = ((activeChar.getY() - World.WORLD_Y_MIN) >> 15) + World.TILE_Y_MIN;
				final int topLeftX = (x - World.TILE_ZERO_COORD_X) * World.TILE_SIZE;
				final int topLeftY = (y - World.TILE_ZERO_COORD_Y) * World.TILE_SIZE;
				final int bottomRightX = (((x - World.TILE_ZERO_COORD_X) * World.TILE_SIZE) + World.TILE_SIZE) - 1;
				final int bottomRightY = (((y - World.TILE_ZERO_COORD_Y) * World.TILE_SIZE) + World.TILE_SIZE) - 1;
				activeChar.sendSysMessage("GeoMap: " + x + "_" + y + " (" + topLeftX + "," + topLeftY + " to " + bottomRightX + "," + bottomRightY + ")");
				final List<Integer> results = new ArrayList<>();
				for (WorldObject obj : World.getInstance().getVisibleObjects())
				{
					if (obj.isNpc() //
						&& !obj.isMonster() //
						&& !(obj.isArtefact()) //
						&& !(obj instanceof BroadcastingTower) //
						&& !(obj instanceof FlyTerrainObject) //
						&& !results.contains(obj.getId()))
					{
						final Npc npc = obj.asNpc();
						if ((npc.getLocation().getX() > topLeftX) && (npc.getLocation().getX() < bottomRightX) && (npc.getLocation().getY() > topLeftY) && (npc.getLocation().getY() < bottomRightY) && npc.isTalkable() && !npc.hasListener(EventType.ON_NPC_FIRST_TALK))
						{
							if ((npc.getHtmlPath(npc.getId(), 0, null).equals("data/html/npcdefault.htm"))//
								|| ((obj instanceof Fisherman) && (HtmCache.getInstance().getHtm(null, "data/html/fisherman/" + npc.getId() + ".htm") == null)) //
								|| ((obj instanceof Warehouse) && (HtmCache.getInstance().getHtm(null, "data/html/warehouse/" + npc.getId() + ".htm") == null)) //
								|| (((obj instanceof Merchant) && !(obj instanceof Fisherman)) && (HtmCache.getInstance().getHtm(null, "data/html/merchant/" + npc.getId() + ".htm") == null)) //
								|| ((obj instanceof Guard) && (HtmCache.getInstance().getHtm(null, "data/html/guard/" + npc.getId() + ".htm") == null)))
							{
								results.add(npc.getId());
							}
						}
					}
				}
				
				Collections.sort(results);
				for (int id : results)
				{
					activeChar.sendSysMessage("NPC " + id + " does not have a default html.");
				}
				
				activeChar.sendSysMessage("Found " + results.size() + " results.");
				break;
			}
			case "admin_world_missing_htmls":
			{
				activeChar.sendSysMessage("Missing htmls for the whole world.");
				final List<Integer> results = new ArrayList<>();
				for (WorldObject obj : World.getInstance().getVisibleObjects())
				{
					if (obj.isNpc() //
						&& !obj.isMonster() //
						&& !(obj.isArtefact()) //
						&& !(obj instanceof BroadcastingTower) //
						&& !(obj instanceof FlyTerrainObject) //
						&& !results.contains(obj.getId()))
					{
						final Npc npc = obj.asNpc();
						if (npc.isTalkable() && !npc.hasListener(EventType.ON_NPC_FIRST_TALK))
						{
							if ((npc.getHtmlPath(npc.getId(), 0, null).equals("data/html/npcdefault.htm")) //
								|| ((obj instanceof Fisherman) && (HtmCache.getInstance().getHtm(null, "data/html/fisherman/" + npc.getId() + ".htm") == null)) //
								|| ((obj instanceof Warehouse) && (HtmCache.getInstance().getHtm(null, "data/html/warehouse/" + npc.getId() + ".htm") == null)) //
								|| (((obj instanceof Merchant) && !(obj instanceof Fisherman)) && (HtmCache.getInstance().getHtm(null, "data/html/merchant/" + npc.getId() + ".htm") == null)) //
								|| ((obj instanceof Guard) && (HtmCache.getInstance().getHtm(null, "data/html/guard/" + npc.getId() + ".htm") == null)))
							{
								results.add(npc.getId());
							}
						}
					}
				}
				
				Collections.sort(results);
				for (int id : results)
				{
					activeChar.sendSysMessage("NPC " + id + " does not have a default html.");
				}
				
				activeChar.sendSysMessage("Found " + results.size() + " results.");
				break;
			}
			case "admin_next_missing_html":
			{
				for (WorldObject obj : World.getInstance().getVisibleObjects())
				{
					if (obj.isNpc() //
						&& !obj.isMonster() //
						&& !(obj.isArtefact()) //
						&& !(obj instanceof BroadcastingTower) //
						&& !(obj instanceof FlyTerrainObject))
					{
						final Npc npc = obj.asNpc();
						if (npc.isTalkable() && !npc.hasListener(EventType.ON_NPC_FIRST_TALK))
						{
							if ((npc.getHtmlPath(npc.getId(), 0, null).equals("data/html/npcdefault.htm")) //
								|| ((obj instanceof Fisherman) && (HtmCache.getInstance().getHtm(null, "data/html/fisherman/" + npc.getId() + ".htm") == null)) //
								|| ((obj instanceof Warehouse) && (HtmCache.getInstance().getHtm(null, "data/html/warehouse/" + npc.getId() + ".htm") == null)) //
								|| (((obj instanceof Merchant) && !(obj instanceof Fisherman)) && (HtmCache.getInstance().getHtm(null, "data/html/merchant/" + npc.getId() + ".htm") == null)) //
								|| ((obj instanceof Guard) && (HtmCache.getInstance().getHtm(null, "data/html/guard/" + npc.getId() + ".htm") == null)))
							{
								activeChar.teleToLocation(npc);
								activeChar.sendSysMessage("NPC " + npc.getId() + " does not have a default html.");
								break;
							}
						}
					}
				}
				break;
			}
		}
		
		return true;
	}
	
	@Override
	public String[] getCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
