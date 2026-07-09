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

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.StringTokenizer;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.cache.HtmCache;
import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.geoengine.geodata.Cell;
import org.l2jmobius.gameserver.geoengine.geodata.IRegion;
import org.l2jmobius.gameserver.geoengine.geodata.regions.NullRegion;
import org.l2jmobius.gameserver.handler.AdminCommandHandler;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;
import org.l2jmobius.gameserver.util.BuilderUtil;
import org.l2jmobius.gameserver.util.GeoUtils;
import org.l2jmobius.gameserver.util.Util;

/**
 * @author Mobius, -Nemesiss-, HorridoJoho
 */
public class AdminGeodata implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_geo_pos",
		"admin_geo_spawn_pos",
		"admin_geo_can_move",
		"admin_geo_can_see",
		"admin_geogrid",
		"admin_geomap",
		"admin_geocell",
		"admin_geosave",
		"admin_geosaveall",
		"admin_geoenablenorth",
		"admin_en",
		"admin_geodisablenorth",
		"admin_dn",
		"admin_geoenablesouth",
		"admin_es",
		"admin_geodisablesouth",
		"admin_ds",
		"admin_geoenableeast",
		"admin_ee",
		"admin_geodisableeast",
		"admin_de",
		"admin_geoenablewest",
		"admin_ew",
		"admin_geodisablewest",
		"admin_dw",
		"admin_geoedit",
		"admin_ge"
	};
	
	@Override
	public boolean useAdminCommand(String command, Player activeChar)
	{
		final StringTokenizer st = new StringTokenizer(command, " ");
		final String actualCommand = st.nextToken();
		switch (actualCommand.toLowerCase())
		{
			case "admin_geo_pos":
			{
				final int worldX = activeChar.getX();
				final int worldY = activeChar.getY();
				final int worldZ = activeChar.getZ();
				final int geoX = GeoEngine.getInstance().getGeoX(worldX);
				final int geoY = GeoEngine.getInstance().getGeoY(worldY);
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					BuilderUtil.sendSysMessage(activeChar, "WorldX: " + worldX + ", WorldY: " + worldY + ", WorldZ: " + worldZ + ", GeoX: " + geoX + ", GeoY: " + geoY + ", GeoZ: " + GeoEngine.getInstance().getHeight(worldX, worldY, worldZ));
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geo_spawn_pos":
			{
				final int worldX = activeChar.getX();
				final int worldY = activeChar.getY();
				final int worldZ = activeChar.getZ();
				final int geoX = GeoEngine.getInstance().getGeoX(worldX);
				final int geoY = GeoEngine.getInstance().getGeoY(worldY);
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					BuilderUtil.sendSysMessage(activeChar, "WorldX: " + worldX + ", WorldY: " + worldY + ", WorldZ: " + worldZ + ", GeoX: " + geoX + ", GeoY: " + geoY + ", GeoZ: " + GeoEngine.getInstance().getHeight(worldX, worldY, worldZ));
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geo_can_move":
			{
				final WorldObject target = activeChar.getTarget();
				if (target != null)
				{
					if (GeoEngine.getInstance().canSeeTarget(activeChar, target))
					{
						BuilderUtil.sendSysMessage(activeChar, "Can move beeline.");
					}
					else
					{
						BuilderUtil.sendSysMessage(activeChar, "Can not move beeline!");
					}
				}
				else
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				}
				break;
			}
			case "admin_geo_can_see":
			{
				final WorldObject target = activeChar.getTarget();
				if (target != null)
				{
					if (GeoEngine.getInstance().canSeeTarget(activeChar, target))
					{
						BuilderUtil.sendSysMessage(activeChar, "Can see target.");
					}
					else
					{
						activeChar.sendPacket(new SystemMessage(SystemMessageId.CANNOT_SEE_TARGET));
					}
				}
				else
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				}
				break;
			}
			case "admin_geogrid":
			{
				if (!st.hasMoreTokens() || !st.nextToken().equalsIgnoreCase("off"))
				{
					GeoUtils.debugGrid(activeChar);
				}
				else
				{
					GeoUtils.hideDebugGrid(activeChar);
				}
				break;
			}
			case "admin_geomap":
			{
				final int x = ((activeChar.getX() - World.WORLD_X_MIN) >> 15) + World.TILE_X_MIN;
				final int y = ((activeChar.getY() - World.WORLD_Y_MIN) >> 15) + World.TILE_Y_MIN;
				BuilderUtil.sendSysMessage(activeChar, "GeoMap: " + x + "_" + y + " (" + ((x - World.TILE_ZERO_COORD_X) * World.TILE_SIZE) + "," + ((y - World.TILE_ZERO_COORD_Y) * World.TILE_SIZE) + " to " + ((((x - World.TILE_ZERO_COORD_X) * World.TILE_SIZE) + World.TILE_SIZE) - 1) + "," + ((((y - World.TILE_ZERO_COORD_Y) * World.TILE_SIZE) + World.TILE_SIZE) - 1) + ")");
				break;
			}
			case "admin_geocell":
			{
				final int geoX = GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = GeoEngine.getInstance().getGeoY(activeChar.getY());
				final int geoZ = GeoEngine.getInstance().getNearestZ(geoX, geoY, activeChar.getZ());
				final int worldX = GeoEngine.getInstance().getWorldX(geoX);
				final int worldY = GeoEngine.getInstance().getWorldY(geoY);
				BuilderUtil.sendSysMessage(activeChar, "GeoCell: " + geoX + ", " + geoY + ". XYZ (" + worldX + ", " + worldY + ", " + geoZ + ")");
				break;
			}
			case "admin_geosave":
			{
				// Create the saves directory if it does not exist.
				final Path savesDir = Config.GEOEDIT_PATH;
				try
				{
					Files.createDirectories(savesDir);
				}
				catch (IOException e)
				{
					BuilderUtil.sendSysMessage(activeChar, "Could not create output directory.");
					return false;
				}
				
				final int x = ((activeChar.getX() - World.WORLD_X_MIN) >> 15) + World.TILE_X_MIN;
				final int y = ((activeChar.getY() - World.WORLD_Y_MIN) >> 15) + World.TILE_Y_MIN;
				final String fileName = String.format(GeoEngine.FILE_NAME_FORMAT, x, y);
				final IRegion region = GeoEngine.getInstance().getRegion(GeoEngine.getInstance().getGeoX(activeChar.getX()), GeoEngine.getInstance().getGeoY(activeChar.getY()));
				if (region instanceof NullRegion)
				{
					BuilderUtil.sendSysMessage(activeChar, "Could not find region: " + x + "_" + y);
				}
				else if (region.saveToFile(fileName))
				{
					BuilderUtil.sendSysMessage(activeChar, "Saved region " + x + "_" + y + " at " + fileName);
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "Could not save region " + x + "_" + y);
				}
				break;
			}
			case "admin_geosaveall":
			{
				// Create the saves directory if it does not exist.
				final Path savesDir = Config.GEOEDIT_PATH;
				try
				{
					Files.createDirectories(savesDir);
				}
				catch (IOException e)
				{
					BuilderUtil.sendSysMessage(activeChar, "Could not create output directory.");
					return false;
				}
				
				int count = 0;
				int worldX = -327680; // Top left Gracia X coord.
				int worldY = -262144; // Top left Gracia Y coord.
				for (int regionX = World.TILE_X_MIN - 1; regionX <= World.TILE_X_MAX; regionX++)
				{
					for (int regionY = World.TILE_Y_MIN - 1; regionY <= World.TILE_Y_MAX; regionY++)
					{
						final int geoX = GeoEngine.getInstance().getGeoX(worldX);
						final int geoY = GeoEngine.getInstance().getGeoY(worldY);
						final IRegion region = GeoEngine.getInstance().getRegion(geoX, geoY);
						if (!(region instanceof NullRegion))
						{
							final int x = ((worldX - World.WORLD_X_MIN) >> 15) + World.TILE_X_MIN;
							final int y = ((worldY - World.WORLD_Y_MIN) >> 15) + World.TILE_Y_MIN;
							final String fileName = String.format(GeoEngine.FILE_NAME_FORMAT, x, y);
							if (region.saveToFile(fileName))
							{
								BuilderUtil.sendSysMessage(activeChar, "Saved region " + x + "_" + y + " at " + fileName);
								count++;
							}
							else
							{
								BuilderUtil.sendSysMessage(activeChar, "Could not save region " + x + "_" + y);
							}
						}
						worldY += World.TILE_SIZE;
					}
					worldX += World.TILE_SIZE;
					worldY = -262144;
				}
				BuilderUtil.sendSysMessage(activeChar, "Saved " + count + " regions.");
				break;
			}
			case "admin_geoenablenorth":
			case "admin_en":
			{
				final int geoX = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getY());
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					GeoEngine.getInstance().setNearestNswe(geoX, geoY, activeChar.getZ(), Cell.NSWE_NORTH);
					if (!actualCommand.contains("geo"))
					{
						AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_ge " + geoX + " " + geoY, false);
					}
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geodisablenorth":
			case "admin_dn":
			{
				final int geoX = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getX());
				final int geoY = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getY());
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					GeoEngine.getInstance().unsetNearestNswe(geoX, geoY, activeChar.getZ(), Cell.NSWE_NORTH);
					if (!actualCommand.contains("geo"))
					{
						AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_ge " + geoX + " " + geoY, false);
					}
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geoenablesouth":
			case "admin_es":
			{
				final int geoX = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getY());
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					GeoEngine.getInstance().setNearestNswe(geoX, geoY, activeChar.getZ(), Cell.NSWE_SOUTH);
					if (!actualCommand.contains("geo"))
					{
						AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_ge " + geoX + " " + geoY, false);
					}
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geodisablesouth":
			case "admin_ds":
			{
				final int geoX = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getY());
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					GeoEngine.getInstance().unsetNearestNswe(geoX, geoY, activeChar.getZ(), Cell.NSWE_SOUTH);
					if (!actualCommand.contains("geo"))
					{
						AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_ge " + geoX + " " + geoY, false);
					}
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geoenableeast":
			case "admin_ee":
			{
				final int geoX = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getY());
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					GeoEngine.getInstance().setNearestNswe(geoX, geoY, activeChar.getZ(), Cell.NSWE_EAST);
					if (!actualCommand.contains("geo"))
					{
						AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_ge " + geoX + " " + geoY, false);
					}
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geodisableeast":
			case "admin_de":
			{
				final int geoX = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getY());
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					GeoEngine.getInstance().unsetNearestNswe(geoX, geoY, activeChar.getZ(), Cell.NSWE_EAST);
					if (!actualCommand.contains("geo"))
					{
						AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_ge " + geoX + " " + geoY, false);
					}
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geoenablewest":
			case "admin_ew":
			{
				final int geoX = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getY());
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					GeoEngine.getInstance().setNearestNswe(geoX, geoY, activeChar.getZ(), Cell.NSWE_WEST);
					if (!actualCommand.contains("geo"))
					{
						AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_ge " + geoX + " " + geoY, false);
					}
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geodisablewest":
			case "admin_dw":
			{
				final int geoX = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = st.hasMoreTokens() ? Integer.parseInt(st.nextToken()) : GeoEngine.getInstance().getGeoY(activeChar.getY());
				if (GeoEngine.getInstance().hasGeoPos(geoX, geoY))
				{
					GeoEngine.getInstance().unsetNearestNswe(geoX, geoY, activeChar.getZ(), Cell.NSWE_WEST);
					if (!actualCommand.contains("geo"))
					{
						AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_ge " + geoX + " " + geoY, false);
					}
				}
				else
				{
					BuilderUtil.sendSysMessage(activeChar, "There is no geodata at this position.");
				}
				break;
			}
			case "admin_geoedit":
			{
				String content = HtmCache.getInstance().getHtm(activeChar, "data/html/admin/geoedit.htm");
				
				// Follow player heading.
				final int direction = getPlayerDirection(activeChar);
				switch (direction)
				{
					case 0: // North.
					{
						content = content.replace("%N%", "N");
						content = content.replace("%E%", "E");
						content = content.replace("%S%", "S");
						content = content.replace("%W%", "W");
						break;
					}
					case 1: // East.
					{
						content = content.replace("%N%", "E");
						content = content.replace("%E%", "S");
						content = content.replace("%S%", "W");
						content = content.replace("%W%", "N");
						break;
					}
					case 2: // South.
					{
						content = content.replace("%N%", "S");
						content = content.replace("%E%", "W");
						content = content.replace("%S%", "N");
						content = content.replace("%W%", "E");
						break;
					}
					default: // West.
					{
						content = content.replace("%N%", "W");
						content = content.replace("%E%", "N");
						content = content.replace("%S%", "E");
						content = content.replace("%W%", "S");
						break;
					}
				}
				
				final int geoRadius = 9;
				final int geoX = GeoEngine.getInstance().getGeoX(activeChar.getX());
				final int geoY = GeoEngine.getInstance().getGeoY(activeChar.getY());
				final int playerZ = activeChar.getZ();
				for (int dx = -geoRadius; dx <= geoRadius; ++dx)
				{
					for (int dy = -geoRadius; dy <= geoRadius; ++dy)
					{
						final int translatedDx;
						final int translatedDy;
						switch (direction)
						{
							case 0: // North.
							{
								translatedDx = dx;
								translatedDy = dy;
								break;
							}
							case 1: // East.
							{
								translatedDx = dy;
								translatedDy = -dx;
								break;
							}
							case 2: // South.
							{
								translatedDx = -dx;
								translatedDy = -dy;
								break;
							}
							default: // West.
							{
								translatedDx = -dy;
								translatedDy = dx;
								break;
							}
						}
						
						final int gx = geoX + dx;
						final int gy = geoY + dy;
						content = content.replace("xy_" + translatedDx + "_" + translatedDy, gx + " " + gy);
						
						final int z = GeoEngine.getInstance().getNearestZ(gx, gy, playerZ);
						final boolean northEnabled = GeoEngine.getInstance().checkNearestNswe(gx, gy, z, Cell.NSWE_NORTH);
						final boolean eastEnabled = GeoEngine.getInstance().checkNearestNswe(gx, gy, z, Cell.NSWE_EAST);
						final boolean southEnabled = GeoEngine.getInstance().checkNearestNswe(gx, gy, z, Cell.NSWE_SOUTH);
						final boolean westEnabled = GeoEngine.getInstance().checkNearestNswe(gx, gy, z, Cell.NSWE_WEST);
						content = content.replace("bg_" + translatedDx + "_" + translatedDy, northEnabled && eastEnabled && southEnabled && westEnabled ? "L2UI_CH3.minibar_food" : "L2UI_CH3.minibar_arrow");
					}
				}
				
				Util.sendCBHtml(activeChar, content);
				break;
			}
			case "admin_ge":
			{
				if (!st.hasMoreTokens())
				{
					AdminCommandHandler.getInstance().useAdminCommand(activeChar, "admin_geoedit", false);
					break;
				}
				
				String content = HtmCache.getInstance().getHtm(activeChar, "data/html/admin/geoedit_cell.htm");
				
				// Follow player heading.
				final int direction = getPlayerDirection(activeChar);
				switch (direction)
				{
					case 0: // North.
					{
						content = content.replace("%", "");
						break;
					}
					case 1: // East.
					{
						content = content.replace("%N%", "E");
						content = content.replace("%E%", "S");
						content = content.replace("%S%", "W");
						content = content.replace("%W%", "N");
						content = content.replace("%bg_n%", "bg_e");
						content = content.replace("%bg_e%", "bg_s");
						content = content.replace("%bg_s%", "bg_w");
						content = content.replace("%bg_w%", "bg_n");
						content = content.replace("%cmd_n%", "cmd_e");
						content = content.replace("%cmd_e%", "cmd_s");
						content = content.replace("%cmd_s%", "cmd_w");
						content = content.replace("%cmd_w%", "cmd_n");
						break;
					}
					case 2: // South.
					{
						content = content.replace("%N%", "S");
						content = content.replace("%E%", "W");
						content = content.replace("%S%", "N");
						content = content.replace("%W%", "E");
						content = content.replace("%bg_n%", "bg_s");
						content = content.replace("%bg_e%", "bg_w");
						content = content.replace("%bg_s%", "bg_n");
						content = content.replace("%bg_w%", "bg_e");
						content = content.replace("%cmd_n%", "cmd_s");
						content = content.replace("%cmd_e%", "cmd_w");
						content = content.replace("%cmd_s%", "cmd_n");
						content = content.replace("%cmd_w%", "cmd_e");
						break;
					}
					default: // West.
					{
						content = content.replace("%N%", "W");
						content = content.replace("%E%", "N");
						content = content.replace("%S%", "E");
						content = content.replace("%W%", "S");
						content = content.replace("%bg_n%", "bg_w");
						content = content.replace("%bg_e%", "bg_n");
						content = content.replace("%bg_s%", "bg_e");
						content = content.replace("%bg_w%", "bg_s");
						content = content.replace("%cmd_n%", "cmd_w");
						content = content.replace("%cmd_e%", "cmd_n");
						content = content.replace("%cmd_s%", "cmd_e");
						content = content.replace("%cmd_w%", "cmd_s");
						break;
					}
				}
				
				final int gx = Integer.parseInt(st.nextToken());
				final int gy = Integer.parseInt(st.nextToken());
				final int z = GeoEngine.getInstance().getNearestZ(gx, gy, activeChar.getZ());
				if (GeoEngine.getInstance().checkNearestNswe(gx, gy, z, Cell.NSWE_NORTH))
				{
					content = content.replace("bg_n", "L2UI_CH3.minibar_food");
					content = content.replace("cmd_n", "dn " + gx + " " + gy);
				}
				else
				{
					content = content.replace("bg_n", "L2UI_CH3.minibar_arrow");
					content = content.replace("cmd_n", "en " + gx + " " + gy);
				}
				if (GeoEngine.getInstance().checkNearestNswe(gx, gy, z, Cell.NSWE_EAST))
				{
					content = content.replace("bg_e", "L2UI_CH3.minibar_food");
					content = content.replace("cmd_e", "de " + gx + " " + gy);
				}
				else
				{
					content = content.replace("bg_e", "L2UI_CH3.minibar_arrow");
					content = content.replace("cmd_e", "ee " + gx + " " + gy);
				}
				if (GeoEngine.getInstance().checkNearestNswe(gx, gy, z, Cell.NSWE_SOUTH))
				{
					content = content.replace("bg_s", "L2UI_CH3.minibar_food");
					content = content.replace("cmd_s", "ds " + gx + " " + gy);
				}
				else
				{
					content = content.replace("bg_s", "L2UI_CH3.minibar_arrow");
					content = content.replace("cmd_s", "es " + gx + " " + gy);
				}
				if (GeoEngine.getInstance().checkNearestNswe(gx, gy, z, Cell.NSWE_WEST))
				{
					content = content.replace("bg_w", "L2UI_CH3.minibar_food");
					content = content.replace("cmd_w", "dw " + gx + " " + gy);
				}
				else
				{
					content = content.replace("bg_w", "L2UI_CH3.minibar_arrow");
					content = content.replace("cmd_w", "ew " + gx + " " + gy);
				}
				
				Util.sendCBHtml(activeChar, content);
				break;
			}
		}
		
		return true;
	}
	
	private int getPlayerDirection(Player activeChar)
	{
		final int heading = activeChar.getHeading();
		if ((heading < 8192) || (heading > 57344))
		{
			return 0; // North.
		}
		else if (heading < 24576)
		{
			return 1; // East.
		}
		else if (heading < 40960)
		{
			return 2; // South.
		}
		else
		{
			return 3; // West.
		}
	}
	
	@Override
	public String[] getAdminCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
