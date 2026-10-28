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

import java.util.StringTokenizer;

import org.l2jmobius.commons.util.StringUtil;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.olympiad.Olympiad;
import org.l2jmobius.gameserver.model.olympiad.OlympiadGameManager;
import org.l2jmobius.gameserver.model.olympiad.OlympiadGameNonClassed;
import org.l2jmobius.gameserver.model.olympiad.OlympiadGameTask;
import org.l2jmobius.gameserver.model.olympiad.OlympiadManager;
import org.l2jmobius.gameserver.model.olympiad.Participant;
import org.l2jmobius.gameserver.network.SystemMessageId;

/**
 * @author UnAfraid, Mobius
 */
public class AdminOlympiad implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_olympiad_game",
		"admin_addolypoints",
		"admin_removeolypoints",
		"admin_setolypoints",
	};
	
	@Override
	public boolean onCommand(String command, Player activeChar)
	{
		final StringTokenizer st = new StringTokenizer(command);
		final String cmd = st.nextToken();
		switch (cmd)
		{
			case "admin_olympiad_game":
			{
				if (!st.hasMoreTokens())
				{
					activeChar.sendSysMessage("Syntax: //olympiad_game <player name>");
					return false;
				}
				
				final Player player = World.getInstance().getPlayer(st.nextToken());
				if (player == null)
				{
					activeChar.sendPacket(SystemMessageId.YOUR_TARGET_CANNOT_BE_FOUND);
					return false;
				}
				
				if (player == activeChar)
				{
					activeChar.sendPacket(SystemMessageId.YOU_CANNOT_USE_THIS_ON_YOURSELF);
					return false;
				}
				
				if (!checkplayer(player, activeChar) || !checkplayer(activeChar, activeChar))
				{
					return false;
				}
				
				for (int i = 0; i < OlympiadGameManager.getInstance().getNumberOfStadiums(); i++)
				{
					final OlympiadGameTask task = OlympiadGameManager.getInstance().getOlympiadTask(i);
					if (task != null)
					{
						synchronized (task)
						{
							if (!task.isRunning())
							{
								final Participant[] players = new Participant[2];
								players[0] = new Participant(activeChar, 1);
								players[1] = new Participant(player, 2);
								task.attachGame(new OlympiadGameNonClassed(i, players));
								return true;
							}
						}
					}
				}
				break;
			}
			case "admin_addolypoints":
			{
				final WorldObject target = activeChar.getTarget();
				final Player player = target != null ? target.asPlayer() : null;
				if (player != null)
				{
					final int val = parseInt(st);
					if (val == -1)
					{
						activeChar.sendSysMessage("Syntax: //addolypoints <points>");
						return false;
					}
					
					if (player.getNobleLevel() > 0)
					{
						final StatSet statDat = getPlayerSet(player);
						final int oldpoints = Olympiad.getInstance().getNoblePoints(player);
						final int points = Math.max(oldpoints + val, 0);
						if (points > 1000)
						{
							activeChar.sendSysMessage("You can't set more than 1000 or less than 0 Olympiad points!");
							return false;
						}
						
						statDat.set(Olympiad.POINTS, points);
						activeChar.sendSysMessage("Player " + player.getName() + " now has " + points + " Olympiad points.");
					}
					else
					{
						activeChar.sendSysMessage("This player is not noblesse!");
						return false;
					}
				}
				else
				{
					activeChar.sendSysMessage("Usage: target a player and write the amount of points you would like to add.");
					activeChar.sendSysMessage("Example: //addolypoints 10");
					activeChar.sendSysMessage("However, keep in mind that you can't have less than 0 or more than 1000 points.");
				}
				break;
			}
			case "admin_removeolypoints":
			{
				final WorldObject target = activeChar.getTarget();
				final Player player = target != null ? target.asPlayer() : null;
				if (player != null)
				{
					final int val = parseInt(st);
					if (val == -1)
					{
						activeChar.sendSysMessage("Syntax: //removeolypoints <points>");
						return false;
					}
					
					if (player.getNobleLevel() > 0)
					{
						final StatSet playerStat = Olympiad.getNobleStats(player.getObjectId());
						if (playerStat == null)
						{
							activeChar.sendSysMessage("This player hasn't played on Olympiad yet!");
							return false;
						}
						
						final int oldpoints = Olympiad.getInstance().getNoblePoints(player);
						final int points = Math.max(oldpoints - val, 0);
						playerStat.set(Olympiad.POINTS, points);
						activeChar.sendSysMessage("Player " + player.getName() + " now has " + points + " Olympiad points.");
					}
					else
					{
						activeChar.sendSysMessage("This player is not noblesse!");
						return false;
					}
				}
				else
				{
					activeChar.sendSysMessage("Usage: target a player and write the amount of points you would like to remove.");
					activeChar.sendSysMessage("Example: //removeolypoints 10");
					activeChar.sendSysMessage("However, keep in mind that you can't have less than 0 or more than 1000 points.");
				}
				break;
			}
			case "admin_setolypoints":
			{
				final WorldObject target = activeChar.getTarget();
				final Player player = target != null ? target.asPlayer() : null;
				if (player != null)
				{
					final int val = parseInt(st);
					if (val == -1)
					{
						activeChar.sendSysMessage("Syntax: //setolypoints <points>");
						return false;
					}
					
					if (player.getNobleLevel() > 0)
					{
						final StatSet statDat = getPlayerSet(player);
						final int oldpoints = Olympiad.getInstance().getNoblePoints(player);
						final int points = oldpoints - val;
						if ((points < 1) && (points > 1000))
						{
							activeChar.sendSysMessage("You can't set more than 1000 or less than 0 Olympiad points! or lower then 0");
							return false;
						}
						
						statDat.set(Olympiad.POINTS, points);
						activeChar.sendSysMessage("Player " + player.getName() + " now has " + points + " Olympiad points.");
					}
					else
					{
						activeChar.sendSysMessage("This player is not noblesse!");
						return false;
					}
				}
				else
				{
					activeChar.sendSysMessage("Usage: target a player and write the amount of points you would like to set.");
					activeChar.sendSysMessage("Example: //setolypoints 10");
					activeChar.sendSysMessage("However, keep in mind that you can't have less than 0 or more than 1000 points.");
				}
				break;
			}
		}
		
		return false;
	}
	
	private int parseInt(StringTokenizer st)
	{
		final String token = st.nextToken();
		if (!StringUtil.isNumeric(token))
		{
			return -1;
		}
		
		return Integer.decode(token);
	}
	
	private StatSet getPlayerSet(Player player)
	{
		StatSet statDat = Olympiad.getNobleStats(player.getObjectId());
		if (statDat == null)
		{
			statDat = new StatSet();
			statDat.set(Olympiad.CLASS_ID, player.getBaseClass());
			statDat.set(Olympiad.CHAR_NAME, player.getName());
			statDat.set(Olympiad.POINTS, Olympiad.DEFAULT_POINTS);
			statDat.set(Olympiad.COMP_DONE, 0);
			statDat.set(Olympiad.COMP_WON, 0);
			statDat.set(Olympiad.COMP_LOST, 0);
			statDat.set(Olympiad.COMP_DRAWN, 0);
			statDat.set(Olympiad.COMP_DONE_WEEK, 0);
			statDat.set("to_save", true);
			Olympiad.addNobleStats(player.getObjectId(), statDat);
		}
		
		return statDat;
	}
	
	private boolean checkplayer(Player player, Player activeChar)
	{
		if (player.isSubClassActive())
		{
			activeChar.sendSysMessage("Player " + player + " subclass active.");
			return false;
		}
		else if (player.getPlayerClass().level() < 3)
		{
			activeChar.sendSysMessage("Player " + player + " has not 3rd class.");
			return false;
		}
		else if (Olympiad.getInstance().getNoblePoints(player) <= 0)
		{
			activeChar.sendSysMessage("Player " + player + " has 0 oly points (add them with (//addolypoints).");
			return false;
		}
		else if (OlympiadManager.getInstance().isRegistered(player))
		{
			activeChar.sendSysMessage("Player " + player + " registered to oly.");
			return false;
		}
		
		return true;
	}
	
	@Override
	public String[] getCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
