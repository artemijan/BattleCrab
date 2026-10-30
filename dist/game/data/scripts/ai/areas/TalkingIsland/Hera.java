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
package ai.areas.TalkingIsland;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

/**
 * Hera AI.
 * @author St3eT
 */
public class Hera extends Script
{
	// NPC
	private static final int HERA = 33208;
	
	private Hera()
	{
		addSpawnId(HERA);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("NPC_SHOUT"))
		{
			npc.broadcastSay(ChatType.NPC_GENERAL, getRandomBoolean() ? NpcStringId.DOES_THE_GODDESS_SEE_WHAT_SHE_HAS_DONE : NpcStringId.WHY_HAVE_THE_HEROES_ABANDONED_US);
			startQuestTimer("NPC_SHOUT", 10000 + (getRandom(5) * 1000), npc, null);
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("NPC_SHOUT", 10000 + (getRandom(5) * 1000), npc, null);
	}
	
	public static void main(String[] args)
	{
		new Hera();
	}
}
