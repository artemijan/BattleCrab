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
package ai.areas.Giran.Kekropus;

import java.util.List;

import org.l2jmobius.gameserver.config.GrandBossConfig;
import org.l2jmobius.gameserver.managers.GrandBossManager;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;

import ai.bosses.Helios.Helios;

/**
 * Kekropus AI
 * @author Gigi
 */
public class Kekropus extends Script
{
	// NPCs
	private static final int KEKROPUS = 34222;
	private static final int HELIOS = 29305;
	
	// Teleports
	private static final Location NORMAL_TELEPORT = new Location(79827, 152588, 2304);
	private static final Location RAID_ENTER_LOC = new Location(79313, 153617, 2307);
	
	// Status
	private static final int ALIVE = 0;
	private static final int WAITING = 1;
	private static final int DEAD = 3;
	
	private Kekropus()
	{
		addStartNpc(KEKROPUS);
		addTalkId(KEKROPUS);
		addFirstTalkId(KEKROPUS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = null;
		switch (event)
		{
			case "teleport":
			{
				player.teleToLocation(NORMAL_TELEPORT);
				break;
			}
			case "helios":
			{
				final int status = GrandBossManager.getInstance().getStatus(HELIOS);
				if (player.isGM())
				{
					player.teleToLocation(RAID_ENTER_LOC, true);
				}
				else
				{
					if ((status > ALIVE) && (status < DEAD))
					{
						return "34222-03.html";
					}
					
					if (status == DEAD)
					{
						return "34222-04.html";
					}
					
					if (!player.isInParty())
					{
						final NpcHtmlMessage packet = new NpcHtmlMessage(npc.getObjectId());
						packet.setHtml(getHtm(player, "34222-01.html"));
						packet.replace("%min%", Integer.toString(GrandBossConfig.HELIOS_MIN_PLAYER));
						packet.replace("%minLevel%", Integer.toString(GrandBossConfig.HELIOS_MIN_PLAYER_LEVEL));
						player.sendPacket(packet);
						return null;
					}
					
					final Party party = player.getParty();
					final boolean isInCC = party.isInCommandChannel();
					final List<Player> members = (isInCC) ? party.getCommandChannel().getMembers() : party.getMembers();
					final boolean isPartyLeader = (isInCC) ? party.getCommandChannel().isLeader(player) : party.isLeader(player);
					if (!isPartyLeader)
					{
						return "34222-02.html";
					}
					
					if (members.size() < GrandBossConfig.HELIOS_MIN_PLAYER)
					{
						final NpcHtmlMessage packet = new NpcHtmlMessage(npc.getObjectId());
						packet.setHtml(getHtm(player, "34222-01.html"));
						packet.replace("%min%", Integer.toString(GrandBossConfig.HELIOS_MIN_PLAYER));
						packet.replace("%minLevel%", Integer.toString(GrandBossConfig.HELIOS_MIN_PLAYER_LEVEL));
						player.sendPacket(packet);
						return null;
					}
					
					for (Player member : members)
					{
						if (member.getLevel() < GrandBossConfig.HELIOS_MIN_PLAYER_LEVEL)
						{
							final NpcHtmlMessage packet = new NpcHtmlMessage(npc.getObjectId());
							packet.setHtml(getHtm(player, "34222-01.html"));
							packet.replace("%min%", Integer.toString(GrandBossConfig.HELIOS_MIN_PLAYER));
							packet.replace("%minLevel%", Integer.toString(GrandBossConfig.HELIOS_MIN_PLAYER_LEVEL));
							player.sendPacket(packet);
							return null;
						}
					}
					
					for (Player member : members)
					{
						if ((member.calculateDistance2D(npc) < 1000) && (npc.getId() == KEKROPUS))
						{
							member.teleToLocation(RAID_ENTER_LOC, true);
						}
					}
				}
				
				if (status == ALIVE)
				{
					GrandBossManager.getInstance().setStatus(HELIOS, WAITING);
					heliosAI().startQuestTimer("beginning", GrandBossConfig.HELIOS_WAIT_TIME * 60000, null, null);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final int i = getRandom(0, 12);
		if ((i > 0) && (i <= 3))
		{
			player.sendPacket(new PlaySound(3, "Npcdialog1.kekrops_greeting_8", 0, 0, 0, 0, 0));
		}
		else if ((i > 3) && (i <= 6))
		{
			player.sendPacket(new PlaySound(3, "Npcdialog1.kekrops_greeting_7", 0, 0, 0, 0, 0));
		}
		else if ((i > 6) && (i <= 9))
		{
			player.sendPacket(new PlaySound(3, "Npcdialog1.kekrops_greeting_6", 0, 0, 0, 0, 0));
		}
		else
		{
			player.sendPacket(new PlaySound(3, "Npcdialog1.kekrops_greeting_5", 0, 0, 0, 0, 0));
		}
		
		return "34222.html";
	}
	
	private Quest heliosAI()
	{
		return ScriptManager.getInstance().getScript(Helios.class.getSimpleName());
	}
	
	public static void main(String[] args)
	{
		new Kekropus();
	}
}
