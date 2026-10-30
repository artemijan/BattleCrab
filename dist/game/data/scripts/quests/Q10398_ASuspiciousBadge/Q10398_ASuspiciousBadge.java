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
package quests.Q10398_ASuspiciousBadge;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * A Suspicious Badge (10398)
 * @author St3eT, Trevor The Third
 */
public class Q10398_ASuspiciousBadge extends Quest
{
	// NPCs
	private static final int ANDY = 33845;
	private static final int BACON = 33846;
	private static final int[] MONSTERS =
	{
		20555, // Giant Fungus
		20558, // Rotting Tree
		23305, // Corroded Skeleton
		23306, // Rotten Corpse
		23307, // Corpse Spider
		23308, // Explosive Spider
	};
	
	// Items
	private static final int BADGE = 36666; // Unidentified Suspicious Badge
	
	// Misc
	private static final int MIN_LEVEL = 52;
	private static final int MAX_LEVEL = 58;
	
	public Q10398_ASuspiciousBadge()
	{
		super(10398);
		addStartNpc(ANDY);
		addTalkId(ANDY, BACON);
		addKillId(MONSTERS);
		registerQuestItems(BADGE);
		addCondNotRace(Race.ERTHEIA, "33845-05.html");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33845-04.htm");
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
			case "33845-02.htm":
			{
				htmltext = event;
				break;
			}
			case "33845-03.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33846-03.html":
			{
				if (qs.isCond(2))
				{
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 6135787, 914);
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
				if (npc.getId() == ANDY)
				{
					htmltext = "33845-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = npc.getId() == ANDY ? "33845-03.html" : "33846-01.html";
				}
				else if (qs.isCond(2) && (npc.getId() == BACON))
				{
					htmltext = "33846-02.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				if (npc.getId() == ANDY)
				{
					htmltext = getAlreadyCompletedMsg(player);
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
		if ((qs != null) && qs.isStarted() && qs.isCond(1))
		{
			if (giveItemRandomly(killer, npc, BADGE, 1, 50, 0.75, true))
			{
				qs.setCond(2);
			}
		}
	}
}
