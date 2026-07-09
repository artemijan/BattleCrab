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
package ai.others.Mammons.MerchantOfMammon;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.util.Broadcast;

import ai.AbstractNpcAI;

/**
 * @author Mobius, Minzee
 */
public class MerchantOfMammon extends AbstractNpcAI
{
	// NPC
	private static final int MERCHANT = 31113;
	// Locations
	private static final Location[] LOCATIONS =
	{
		new Location(-52172, 78884, -4741, 0), // Devotion
		new Location(-41350, 209876, -5087, 0), // Sacrifice
		new Location(-21657, 77164, -5173, 0), // Patriots
		new Location(45029, 123802, -5413, 0), // Pilgrims
		new Location(83175, 208998, -5439, 0), // Saints
		new Location(111337, 173804, -5439, 0), // Worship
		new Location(118343, 132578, -4831, 0), // Martyrdom
		new Location(172373, -17833, -4901, 0), // Disciple
	};
	// Misc
	private static final int TELEPORT_DELAY = 1800000; // 30 minutes
	private static Npc _lastSpawn;
	
	private MerchantOfMammon()
	{
		addFirstTalkId(MERCHANT);
		onEvent("RESPAWN_MERCHANT", null, null);
		startQuestTimer("RESPAWN_MERCHANT", TELEPORT_DELAY, null, null, true);
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
			case "RESPAWN_MERCHANT":
			{
				if (_lastSpawn != null)
				{
					_lastSpawn.deleteMe();
				}
				_lastSpawn = addSpawn(MERCHANT, getRandomEntry(LOCATIONS), false, TELEPORT_DELAY);
				if (Config.ANNOUNCE_MAMMON_SPAWN)
				{
					Broadcast.toAllOnlinePlayers("Merchant of Mammon has been spawned near the Town of " + _lastSpawn.getCastle().getName() + ".", false);
				}
				break;
			}
		}
		return htmltext;
	}
	
	public static void main(String[] args)
	{
		new MerchantOfMammon();
	}
}
