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
package ai.others.Mammons;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;

import org.l2jmobius.gameserver.config.NpcConfig;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.util.Broadcast;

/**
 * Mammons AI.
 * @author St3eT
 */
public class Mammons extends Script
{
	// NPCs
	private static final int[] MAMMONS =
	{
		31126, // Blacksmith of Mammon
		33739, // Priest of Mammon
		33511, // Merchant of Mammon
	};
	
	// Locations
	private static final Location[] BLACKSMITH_LOC =
	{
		new Location(146873, 29448, -2264, 0), // Aden
		new Location(81266, 150091, -3528, 891), // Giran
		new Location(42825, -41337, -2184), // Rune
	};
	private static final Location[] MERCHANT_LOC =
	{
		new Location(146872, 29569, -2264, 0), // Aden
		new Location(81272, 150041, -3528, 891), // Giran
		new Location(42803, -41283, -2184, 37972), // Rune
	};
	private static final Location[] PRIEST_LOC =
	{
		new Location(146882, 29665, -2264, 0), // Aden
		new Location(81284, 150155, -3528, 891), // Giran
		new Location(42784, -41236, -2192, 37972), // Rune
	};
	
	// Misc
	private static final NpcStringId[] RANDOM_SAY =
	{
		NpcStringId.I_BRING_YOU_WONDROUS_GIFTS,
		NpcStringId.I_HAVE_SOME_EXCELLENT_WEAPONS_TO_SHOW_YOU,
		NpcStringId.I_VE_BEEN_SO_BUSY_LATELY_IN_ADDITION_TO_PLANNING_MY_TRIP,
	};
	private static final int TELEPORT_DELAY = 3600000; // 60min
	private static final List<Npc> _mammons = new ArrayList<>();
	
	private Mammons()
	{
		addStartNpc(MAMMONS);
		addTalkId(MAMMONS);
		addFirstTalkId(MAMMONS);
		
		onEvent("RESPAWN_MAMMONS", null, null);
		startQuestTimer("RESPAWN_MAMMONS", TELEPORT_DELAY, null, null, true);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "31126.html":
			case "31126-01.html":
			case "31126-02.html":
			case "31126-03.html":
			case "31126-04.html":
			case "31126-05.html":
			case "31126-06.html":
			case "33739-01.html":
			{
				htmltext = event;
				break;
			}
			case "RESPAWN_MAMMONS":
			{
				if (!_mammons.isEmpty())
				{
					_mammons.stream().filter(Objects::nonNull).forEach(Npc::deleteMe);
					_mammons.clear();
				}
				
				final int town = getRandom(3);
				final Npc blacksmith = addSpawn(MAMMONS[0], BLACKSMITH_LOC[town]);
				final Npc merchant = addSpawn(MAMMONS[1], MERCHANT_LOC[town]);
				final Npc priest = addSpawn(MAMMONS[2], PRIEST_LOC[town]);
				_mammons.addAll(Arrays.asList(blacksmith, merchant, priest));
				if (blacksmith != null)
				{
					blacksmith.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(RANDOM_SAY));
					if (NpcConfig.ANNOUNCE_MAMMON_SPAWN)
					{
						Broadcast.toAllOnlinePlayers("Mammon's has been spawned in Town of " + blacksmith.getCastle().getName() + ".", false);
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	public static void main(String[] args)
	{
		new Mammons();
	}
}
