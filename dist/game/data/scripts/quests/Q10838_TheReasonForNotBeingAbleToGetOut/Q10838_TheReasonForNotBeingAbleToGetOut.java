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
package quests.Q10838_TheReasonForNotBeingAbleToGetOut;

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
 * The Reason For Not Being Able to Get Out (10838)
 * @URL https://l2wiki.com/The_Reason_For_Not_Being_Able_to_Get_Out
 * @author Gigi
 */
public class Q10838_TheReasonForNotBeingAbleToGetOut extends Quest
{
	// NPC
	private static final int HURAK = 34064;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23506, // Fortress Guardian Captain
		23505, // Fortress Raider
		23507, // Atelia Passionate Soldier
		23508 // Atelia Elite Captain
	};
	
	// Items
	private static final int BLACKBIRD_REPORT_HURAK = 46135;
	private static final int BLACKBIRD_SEAL = 46132;
	private static final int DARK_ATELIA_NATURALIZER = 46133;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q10838_TheReasonForNotBeingAbleToGetOut()
	{
		super(10838);
		addStartNpc(HURAK);
		addTalkId(HURAK);
		addKillId(MONSTERS);
		registerQuestItems(DARK_ATELIA_NATURALIZER);
		addCondMinLevel(MIN_LEVEL, "34064-00.htm");
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
			case "34064-02.htm":
			case "34064-04.htm":
			{
				htmltext = event;
				break;
			}
			case "34064-05.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34064-08.html":
			{
				giveItems(player, BLACKBIRD_REPORT_HURAK, 1);
				addExpAndSp(player, 9683068920L, 23239200);
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
					htmltext = "34064-03.htm";
					break;
				}
				
				htmltext = "34064-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34064-06.html";
				}
				else
				{
					htmltext = "34064-07.html";
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
			if ((count >= 150) && (getQuestItemsCount(killer, DARK_ATELIA_NATURALIZER) >= 10))
			{
				qs.setCond(2, true);
			}
			else
			{
				if ((getQuestItemsCount(killer, DARK_ATELIA_NATURALIZER) < 10) && (getRandom(100) > 90))
				{
					giveItems(killer, DARK_ATELIA_NATURALIZER, 1);
				}
				
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
				holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_THE_EMBRYO, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
