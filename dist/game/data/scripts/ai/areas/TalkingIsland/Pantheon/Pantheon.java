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
package ai.areas.TalkingIsland.Pantheon;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.Q10320_LetsGoToTheCentralSquare.Q10320_LetsGoToTheCentralSquare;

/**
 * Pantheon AI.
 * @author Gladicek
 */
public class Pantheon extends Script
{
	// NPC
	private static final int PANTHEON = 32972;
	
	// Location
	private static final Location MUSEUM = new Location(-114711, 243911, -7968);
	
	// Misc
	private static final int MIN_LEVEL = 20;
	
	private Pantheon()
	{
		addSpawnId(PANTHEON);
		addStartNpc(PANTHEON);
		addFirstTalkId(PANTHEON);
		addTalkId(PANTHEON);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "32972-1.html":
			{
				htmltext = event;
				break;
			}
			case "teleport_museum":
			{
				if (player.getLevel() < MIN_LEVEL)
				{
					htmltext = "32972-noteleport.html";
				}
				else
				{
					player.teleToLocation(MUSEUM);
				}
				break;
			}
			case "TEXT_SPAM":
			{
				npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.IS_IT_BETTER_TO_END_DESTINY_OR_START_DESTINY);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final QuestState qs = player.getQuestState(Q10320_LetsGoToTheCentralSquare.class.getSimpleName());
		if (qs == null)
		{
			showOnScreenMsg(player, NpcStringId.BEGIN_TUTORIAL_QUESTS, ExShowScreenMessage.TOP_CENTER, 4500);
		}
		
		return npc.getId() + ".html";
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("TEXT_SPAM", 10000, npc, null, true);
	}
	
	public static void main(String[] args)
	{
		new Pantheon();
	}
}
