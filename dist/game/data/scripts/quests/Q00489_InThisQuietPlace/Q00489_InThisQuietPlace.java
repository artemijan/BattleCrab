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
package quests.Q00489_InThisQuietPlace;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * In This Quiet Place (489)
 * @author St3eT
 */
public class Q00489_InThisQuietPlace extends Quest
{
	// NPCs
	private static final int ADVENTURER = 32327;
	private static final int BESTIAN = 31280;
	private static final int[] MONSTERS =
	{
		21646, // Grave Scarab
		21647, // Scavenger Scarab
		21648, // Grave Ant
		21649, // Scavenger Ant
		21650, // Shrine Knight
		21651, // Shrine Guard
	};
	
	// Items
	private static final int EVIL_SPIRIT = 19501; // Trace of Evil Spirit
	
	// Misc
	private static final int MIN_LEVEL = 75;
	private static final int MAX_LEVEL = 79;
	
	public Q00489_InThisQuietPlace()
	{
		super(489);
		addStartNpc(ADVENTURER);
		addTalkId(ADVENTURER, BESTIAN);
		addKillId(MONSTERS);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "");
		registerQuestItems(EVIL_SPIRIT);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = null;
		switch (event)
		{
			case "32327-02.htm":
			case "32327-03.htm":
			{
				htmltext = event;
				break;
			}
			case "32327-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == ADVENTURER)
				{
					htmltext = "32327-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = npc.getId() == ADVENTURER ? "32327-05.html" : "31280-01.html";
				}
				else if (qs.isCond(2))
				{
					if (npc.getId() == ADVENTURER)
					{
						htmltext = "32327-06.html";
					}
					else if (npc.getId() == BESTIAN)
					{
						qs.exitQuest(QuestType.DAILY, true);
						giveAdena(player, 426_045, true);
						if (player.getLevel() >= MIN_LEVEL)
						{
							addExpAndSp(player, 19_890_000, 4_773);
						}
						
						htmltext = "32180-02.html";
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if ((npc.getId() == ADVENTURER) && qs.isNowAvailable())
				{
					qs.setState(State.CREATED);
					htmltext = "32327-01.htm";
				}
				else if ((npc.getId() == BESTIAN) && qs.isCompleted() && !qs.isNowAvailable())
				{
					htmltext = "32180-03.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			if (giveItemRandomly(killer, EVIL_SPIRIT, 1, 77, 0.4, true))
			{
				qs.setCond(2, true);
			}
		}
	}
}
