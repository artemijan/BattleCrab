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
package quests.Q10840_TimeToRecover;

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

/**
 * Time to Recover (10840)
 * @URL https://l2wiki.com/Time_to_Recover
 * @author Gigi
 */
public class Q10840_TimeToRecover extends Quest
{
	// NPC
	private static final int SHERRY = 34066;
	private static final int[] MONSTERS =
	{
		23512, // Atelia High Priest
		23509, // Fortress Dark Wizard
		23507, // Atelia Passionate Soldier
		23508, // Atelia Elite Captain
		23510, // Atelia Flame Master
		23511 // Fortress Archon
	};
	
	// Items
	private static final int BLACKBIRD_REPORT_SHERRY = 46137;
	private static final int BLACKBIRD_SEAL = 46132;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q10840_TimeToRecover()
	{
		super(10840);
		addStartNpc(SHERRY);
		addTalkId(SHERRY);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "34066-00.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "34066-02.htm":
			case "34066-04.htm":
			{
				htmltext = event;
				break;
			}
			case "34066-05.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34066-08.html":
			{
				giveItems(player, BLACKBIRD_REPORT_SHERRY, 1);
				addExpAndSp(player, 14524603380L, 34858800);
				qs.exitQuest(false, true);
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
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (!hasQuestItems(player, BLACKBIRD_SEAL))
				{
					htmltext = "34066-03.htm";
					break;
				}
				
				htmltext = "34066-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34066-06.html";
				}
				else
				{
					htmltext = "34066-07.html";
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
		if ((qs != null) && qs.isCond(1))
		{
			int count = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++count);
			if (count >= 250)
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
				holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_THE_EMBRYO_3, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
