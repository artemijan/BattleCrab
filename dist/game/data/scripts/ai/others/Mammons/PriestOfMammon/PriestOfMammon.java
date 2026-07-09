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
package ai.others.Mammons.PriestOfMammon;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.util.Broadcast;

import ai.AbstractNpcAI;

/**
 * @author Mobius, Minzee
 */
public class PriestOfMammon extends AbstractNpcAI
{
	// NPC
	private static final int PRIEST = 33511;
	// Locations
	private static final Location[] LOCATIONS =
	{
		new Location(146882, 29665, -2264, 0), // Aden
		new Location(81284, 150155, -3528, 891), // Giran
		new Location(42784, -41236, -2192, 37972), // Rune
	};
	// Misc
	private static final int TELEPORT_DELAY = 1800000; // 30 minutes
	private static Npc _lastSpawn;
	
	private PriestOfMammon()
	{
		addFirstTalkId(PRIEST);
		onEvent("RESPAWN_PRIEST", null, null);
		startQuestTimer("RESPAWN_PRIEST", TELEPORT_DELAY, null, null, true);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "31113.html":
			case "31113-01.html":
			case "31113-02.html":
			{
				htmltext = event;
				break;
			}
			case "RESPAWN_PRIEST":
			{
				if (_lastSpawn != null)
				{
					_lastSpawn.deleteMe();
				}
				_lastSpawn = addSpawn(PRIEST, getRandomEntry(LOCATIONS), false, TELEPORT_DELAY);
				if (Config.ANNOUNCE_MAMMON_SPAWN)
				{
					Broadcast.toAllOnlinePlayers("Priest of Mammon has been spawned in Town of " + _lastSpawn.getCastle().getName() + ".", false);
				}
				break;
			}
		}
		return htmltext;
	}
	
	public static void main(String[] args)
	{
		new PriestOfMammon();
	}
}
