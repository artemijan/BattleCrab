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
package quests.Q10399_TheAlphabetOfTheGiants;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10398_ASuspiciousBadge.Q10398_ASuspiciousBadge;

/**
 * The Alphabet of the Giants (10399)
 * @author St3eT
 */
public class Q10399_TheAlphabetOfTheGiants extends Quest
{
	// NPCs
	private static final int BACON = 33846;
	private static final int[] MONSTERS =
	{
		23309, // Corpse Looter Stakato
		23310, // Lesser Laikel
	};
	
	// Items
	private static final int TABLET = 36667; // Giant's Alphabet
	
	// Misc
	private static final int MIN_LEVEL = 52;
	private static final int MAX_LEVEL = 58;
	
	public Q10399_TheAlphabetOfTheGiants()
	{
		super(10399);
		addStartNpc(BACON);
		addTalkId(BACON);
		addKillId(MONSTERS);
		registerQuestItems(TABLET);
		addCondNotRace(Race.ERTHEIA, "33846-08.html");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33846-07.htm");
		addCondCompletedQuest(Q10398_ASuspiciousBadge.class.getSimpleName(), "33846-07.htm");
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
			case "33846-02.htm":
			{
				htmltext = event;
				break;
			}
			case "33846-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33846-06.html":
			{
				if (qs.isCond(2))
				{
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 8779765, 914);
					}
					
					htmltext = event;
				}
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
				htmltext = "33846-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "33846-04.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "33846-05.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isStarted() && qs.isCond(1))
		{
			if (giveItemRandomly(killer, npc, TABLET, 1, 50, 1, true))
			{
				qs.setCond(2);
			}
		}
	}
}
