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
package ai.areas.DragonValley.MercenaryTeleport;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Mercenary and Mercenary Captain teleport AI.
 * @author Gigi
 */
public class MercenaryTeleport extends Script
{
	// NPCs
	private static final int NAMO = 33973;
	private static final int MERCENARY = 33971;
	private static final int MERCENARY_CAPTAIN = 33970;
	
	// Locations
	private static final Map<String, Location> LOCATIONS = new HashMap<>();
	static
	{
		// Captain
		LOCATIONS.put("NorthernDragonValley", new Location(87712, 106060, -3176));
		LOCATIONS.put("SouthernDragonValley", new Location(88016, 118852, -3056));
		LOCATIONS.put("NorthernWhirlingVortex", new Location(108064, 112432, -3008));
		LOCATIONS.put("SouthernWhirlingVortex", new Location(109918, 121266, -3720));
		LOCATIONS.put("DeepInWhirlingVortex", new Location(119506, 112331, -3688));
		LOCATIONS.put("EntranceToAntharasLair", new Location(131116, 114333, -3704));
		LOCATIONS.put("AntharasLairBarrierBridge", new Location(146129, 111232, -3568));
		LOCATIONS.put("DeepInAntharasLair", new Location(148447, 110582, -3944));
		// Mercenary
		LOCATIONS.put("TownOfGiran", new Location(83497, 148015, -3400));
		LOCATIONS.put("DragonValleyJunction", new Location(80012, 115911, -3672));
		LOCATIONS.put("WhirlingVortexJunction", new Location(102278, 113038, -3720));
	}
	
	private MercenaryTeleport()
	{
		addStartNpc(NAMO, MERCENARY, MERCENARY_CAPTAIN);
		addFirstTalkId(NAMO, MERCENARY, MERCENARY_CAPTAIN);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		player.teleToLocation(LOCATIONS.get(event), true);
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	public static void main(String[] args)
	{
		new MercenaryTeleport();
	}
}
