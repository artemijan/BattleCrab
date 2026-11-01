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
package quests.Q10366_ReportOnTheSituationAtTheRuins;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

import quests.Q10365_ForTheSearchdogKing.Q10365_ForTheSearchdogKing;

/**
 * Report on the situation at the ruins (10366)
 * @URL https://l2wiki.com/Report_on_the_situation_at_the_ruins
 * @author Gigi
 */
public class Q10366_ReportOnTheSituationAtTheRuins extends Quest
{
	// NPCs
	private static final int SEBION = 32978;
	private static final int[] MONSTERS =
	{
		22993, // Critter
		22994, // Avian
		22995, // Rider
		23122 // Eyesaroch
	};
	
	// Misc
	private static final int MIN_LEVEL = 17;
	private static final int MAX_LEVEL = 25;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q10366_ReportOnTheSituationAtTheRuins()
	{
		super(10366);
		addStartNpc(SEBION);
		addTalkId(SEBION);
		addKillId(MONSTERS);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "32978-08.html");
		addCondCompletedQuest(Q10365_ForTheSearchdogKing.class.getSimpleName(), "32978-08.html");
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
			case "32978-02.htm":
			case "32978-02a.htm":
			{
				htmltext = event;
				break;
			}
			case "32978-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32978-06.html":
			{
				if (qs.isCond(2))
				{
					addExpAndSp(player, 114000, 15);
					qs.exitQuest(false, true);
				}
				
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = null;
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "32978-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "32978-04.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "32978-05.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "32978-07.html";
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
			int killCount = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++killCount);
			if (killCount >= 40)
			{
				qs.setCond(2, true);
			}
			else
			{
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final int killCount = qs.getInt(KILL_COUNT_VAR);
			if (killCount > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_THE_HUSKS, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
