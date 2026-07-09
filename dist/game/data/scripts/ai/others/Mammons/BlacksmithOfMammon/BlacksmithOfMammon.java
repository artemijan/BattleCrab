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
package ai.others.Mammons.BlacksmithOfMammon;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.util.Broadcast;

import ai.AbstractNpcAI;

/**
 * @author Mobius, Minzee
 */
public class BlacksmithOfMammon extends AbstractNpcAI
{
	// NPC
	private static final int BLACKSMITH = 31126;
	// Locations
	private static final Location[] LOCATIONS =
	{
		new Location(-19360, 13278, -4901, 0), // Dark Omens
		new Location(-53131, -250502, -7909, 0), // Heretic
		new Location(46303, 170091, -4981, 0), // Branded
		new Location(-20485, -251008, -8165, 0), // Apostate
		new Location(12669, -248698, -9581, 0), // Forbidden Path
		new Location(140519, 79464, -5429, 0), // Witch
	};
	// Misc
	private static final int TELEPORT_DELAY = 1800000; // 30 minutes
	private static Npc _lastSpawn;
	
	private BlacksmithOfMammon()
	{
		addFirstTalkId(BLACKSMITH);
		onEvent("RESPAWN_BLACKSMITH", null, null);
		startQuestTimer("RESPAWN_BLACKSMITH", TELEPORT_DELAY, null, null, true);
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
			{
				htmltext = event;
				break;
			}
			case "RESPAWN_BLACKSMITH":
			{
				if (_lastSpawn != null)
				{
					_lastSpawn.deleteMe();
				}
				_lastSpawn = addSpawn(BLACKSMITH, getRandomEntry(LOCATIONS), false, TELEPORT_DELAY);
				if (Config.ANNOUNCE_MAMMON_SPAWN)
				{
					Broadcast.toAllOnlinePlayers("Blacksmith of Mammon has been spawned near the Town of " + _lastSpawn.getCastle().getName() + ".", false);
				}
				break;
			}
		}
		return htmltext;
	}
	
	public static void main(String[] args)
	{
		new BlacksmithOfMammon();
	}
}
