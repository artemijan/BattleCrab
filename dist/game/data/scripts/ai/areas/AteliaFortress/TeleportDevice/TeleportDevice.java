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
package ai.areas.AteliaFortress.TeleportDevice;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Kingdom's Royal Guard Teleport Device
 * @author Gigi
 * @date 2018-04-30 - [23:32:48]
 */
public class TeleportDevice extends Script
{
	// NPC
	private static final int TELEPORT_DEVICE = 34242;
	
	// Teleport's
	private static final Location LOCATION1 = new Location(-46335, 59575, -2960);
	private static final Location LOCATION2 = new Location(-42307, 51232, -2032);
	private static final Location LOCATION3 = new Location(-44060, 40139, -1432);
	private static final Location LOCATION4 = new Location(-57242, 43811, -1552);
	
	private TeleportDevice()
	{
		addFirstTalkId(TELEPORT_DEVICE);
		addTalkId(TELEPORT_DEVICE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 3)
		{
			return "34242-01.html";
		}
		
		switch (event)
		{
			case "teleport1":
			{
				player.teleToLocation(LOCATION1);
				break;
			}
			case "teleport2":
			{
				player.teleToLocation(LOCATION2);
				break;
			}
			case "teleport3":
			{
				player.teleToLocation(LOCATION3);
				break;
			}
			case "teleport4":
			{
				player.teleToLocation(LOCATION4);
				break;
			}
		}
		
		return null;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "34242.html";
	}
	
	public static void main(String[] args)
	{
		new TeleportDevice();
	}
}
