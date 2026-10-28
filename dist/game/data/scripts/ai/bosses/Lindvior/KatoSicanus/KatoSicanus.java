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
package ai.bosses.Lindvior.KatoSicanus;

import java.util.List;

import org.l2jmobius.gameserver.config.GrandBossConfig;
import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.managers.GrandBossManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

/**
 * Kato Sicanus Teleporter AI
 * @author Gigi
 * @date 2017-07-13 - [22:17:16]
 */
public class KatoSicanus extends Script
{
	// NPCs
	private static final int KATO_SICANUS = 33881;
	private static final int LINDVIOR_RAID = 29240;
	private static final int INVISIBLE = 8572;
	
	// Location
	private static final Location LINDVIOR_LOCATION = new Location(46929, -28807, -1400);
	
	public KatoSicanus()
	{
		addFirstTalkId(KATO_SICANUS);
		addTalkId(KATO_SICANUS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("teleport"))
		{
			final int status = GrandBossManager.getInstance().getStatus(LINDVIOR_RAID);
			if (player.isGM())
			{
				player.teleToLocation(LINDVIOR_LOCATION, true);
				addSpawn(INVISIBLE, 46707, -28586, -1400, 0, false, 60000, false);
				GrandBossManager.getInstance().setStatus(LINDVIOR_RAID, 1);
			}
			else
			{
				if (status == 2)
				{
					return "33881-1.html";
				}
				
				if (status == 3)
				{
					return "33881-2.html";
				}
				
				if (!player.isInParty())
				{
					return "33881-3.html";
				}
				
				final Party party = player.getParty();
				final boolean isInCC = party.isInCommandChannel();
				final List<Player> members = (isInCC) ? party.getCommandChannel().getMembers() : party.getMembers();
				final boolean isPartyLeader = (isInCC) ? party.getCommandChannel().isLeader(player) : party.isLeader(player);
				for (Player member : members)
				{
					if (!member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
					{
						return "33881-4.html";
					}
				}
				
				if (!isPartyLeader)
				{
					return "33881-3.html";
				}
				
				if ((members.size() < GrandBossConfig.LINDVIOR_MIN_PLAYERS) || (members.size() > GrandBossConfig.LINDVIOR_MAX_PLAYERS))
				{
					final NpcHtmlMessage packet = new NpcHtmlMessage(npc.getObjectId());
					packet.setHtml(getHtm(player, "33881-4.html"));
					packet.replace("%min%", Integer.toString(GrandBossConfig.LINDVIOR_MIN_PLAYERS));
					packet.replace("%max%", Integer.toString(GrandBossConfig.LINDVIOR_MAX_PLAYERS));
					player.sendPacket(packet);
					return null;
				}
				
				for (Player member : members)
				{
					if (member.getLevel() < GrandBossConfig.LINDVIOR_MIN_PLAYER_LEVEL)
					{
						final NpcHtmlMessage packet = new NpcHtmlMessage(npc.getObjectId());
						packet.setHtml(getHtm(player, "33881-5.html"));
						packet.replace("%minLevel%", Integer.toString(GrandBossConfig.LINDVIOR_MIN_PLAYER_LEVEL));
						player.sendPacket(packet);
						return null;
					}
				}
				
				for (Player member : members)
				{
					if (member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
					{
						member.teleToLocation(LINDVIOR_LOCATION, true);
						addSpawn(INVISIBLE, 46707, -28586, -1400, 0, false, 0, false);
						GrandBossManager.getInstance().setStatus(LINDVIOR_RAID, 1);
					}
				}
			}
		}
		
		return null;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "33881.html";
	}
	
	public static void main(String[] args)
	{
		new KatoSicanus();
	}
}
