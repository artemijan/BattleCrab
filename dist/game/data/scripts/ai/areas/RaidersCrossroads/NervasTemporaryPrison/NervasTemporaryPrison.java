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
package ai.areas.RaidersCrossroads.NervasTemporaryPrison;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Door;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

/**
 * @author Index
 */
public class NervasTemporaryPrison extends Script
{
	// NPCs
	private static final int KAYSEN = 19458;
	private static final int NERVAS_TEMPORARY_PRISON = 19459;
	
	// Locations
	private static final Location[] SPAWN_LOCATIONS =
	{
		new Location(10595, -136216, -1192),
		new Location(11924, -141102, -592),
		new Location(18263, -137084, -896),
		new Location(19991, -142252, -576),
		new Location(22752, -139032, -744),
		new Location(23220, -146000, -464),
		new Location(6516, -139680, -656),
		new Location(8555, -146514, -312),
	};
	
	// Item
	private static final int NERVA_KEY = 36665;
	
	private NervasTemporaryPrison()
	{
		addStartNpc(NERVAS_TEMPORARY_PRISON);
		addFirstTalkId(NERVAS_TEMPORARY_PRISON);
		addTalkId(NERVAS_TEMPORARY_PRISON);
		addSpawnId(NERVAS_TEMPORARY_PRISON);
		
		for (Location location : SPAWN_LOCATIONS)
		{
			addSpawn(NERVAS_TEMPORARY_PRISON, location);
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "OPEN":
			{
				if (hasQuestItems(player, NERVA_KEY))
				{
					for (Door door : World.getInstance().getVisibleObjectsInRange(npc, Door.class, Npc.INTERACTION_DISTANCE))
					{
						door.openMe();
					}
					
					for (Npc nearby : World.getInstance().getVisibleObjectsInRange(npc, Npc.class, Npc.INTERACTION_DISTANCE))
					{
						if (nearby.getId() == KAYSEN)
						{
							nearby.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_TOOK_DOWN_THE_NERVA_ORCS_AND_GOT_THEIR_TEMPORARY_PRISON_KEY);
							break;
						}
					}
					
					takeItems(player, NERVA_KEY, 1);
					
					npc.deleteMe();
					startQuestTimer("PRISON_RESPAWN", 3600000, npc, null);
				}
				else
				{
					return "19459-no.html";
				}
				break;
			}
			case "PRISON_RESPAWN":
			{
				addSpawn(NERVAS_TEMPORARY_PRISON, npc);
				break;
			}
		}
		
		return null;
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		for (Door door : World.getInstance().getVisibleObjectsInRange(npc, Door.class, Npc.INTERACTION_DISTANCE))
		{
			door.closeMe();
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	public static void main(String[] args)
	{
		new NervasTemporaryPrison();
	}
}
