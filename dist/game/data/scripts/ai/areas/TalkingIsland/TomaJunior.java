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
 * Guard Soldier AI.
 * @author Gladicek
 */
public class TomaJunior extends Script
{
	// NPCs
	private static final int TOMA_JUNIOR = 33571;
	
	private TomaJunior()
	{
		addSpawnId(TOMA_JUNIOR);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("SPAM_TEXT") && (npc != null))
		{
			npc.broadcastSocialAction(3);
			npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THE_BEACH_WHERE_RELICS_OF_GIANTS_HAD_FALLEN_HAS_NOW_BECOME_CLEAN, 1000);
		}
		else if (event.equals("SOCIAL_ACTION") && (npc != null))
		{
			npc.broadcastSocialAction(6);
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("SPAM_TEXT", 9000, npc, null, true);
		startQuestTimer("SOCIAL_ACTION", 15000, npc, null, true);
	}
	
	public static void main(String[] args)
	{
		new TomaJunior();
	}
}
