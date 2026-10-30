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
package quests.Q00483_IntendedTactic;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Intended Tactic (483)
 * @URL https://l2wiki.com/Intended_Tactic
 * @author Gigi
 */
public class Q00483_IntendedTactic extends Quest
{
	// NPC
	private static final int ENDE = 33357;
	
	// Monsters
	private static final int[] MOBS =
	{
		23069, // Vladimir's Warrior
		23070, // Lazearth' Warrior
		23071, // Beastian
		23072, // Birestian
		23073, // Kenneth Bastian
		23074, // Heaven's Palace Noble Warrior
		23075 // Heaven's Palace Noble Knight
	};
	private static final int[] BOSSES =
	{
		25809, // Vladimir
		25811, // Lazearth
		25815 // Ken
	};
	
	// Items
	private static final int LOYAL_SERVANS_BLOOD = 17736;
	private static final int TRUTTHFUL_ONES_BLOOD = 17737;
	private static final int TOKEN_OF_INSOLENCE_TOWER = 17624;
	
	// Misc
	private static final int MIN_LEVEL = 48;
	
	public Q00483_IntendedTactic()
	{
		super(483);
		addStartNpc(ENDE);
		addTalkId(ENDE);
		addKillId(MOBS);
		addKillId(BOSSES);
		addCondMinLevel(MIN_LEVEL, "33357-02.htm");
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
			case "33357-05.htm":
			case "33357-06.htm":
			case "33357-07.htm":
			{
				htmltext = event;
				break;
			}
			case "33357-08.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "endquest":
			{
				if (getQuestItemsCount(player, TRUTTHFUL_ONES_BLOOD) >= 10)
				{
					takeItems(player, LOYAL_SERVANS_BLOOD, -1);
					takeItems(player, TRUTTHFUL_ONES_BLOOD, -1);
					giveItems(player, TOKEN_OF_INSOLENCE_TOWER, 1);
					addExpAndSp(player, 1500000, 360);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = "33357-12.html";
					break;
				}
				
				takeItems(player, LOYAL_SERVANS_BLOOD, -1);
				addExpAndSp(player, 1500000, 360);
				qs.exitQuest(QuestType.DAILY, true);
				htmltext = "33357-11.html";
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
		if (npc.getId() == ENDE)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "33357-03.html";
						break;
					}
					
					qs.setState(State.CREATED);
					break;
				}
				case State.CREATED:
				{
					htmltext = "33357-01.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "33357-09.html";
					}
					else if (qs.isStarted() && qs.isCond(2))
					{
						htmltext = "33357-10.html";
					}
					break;
				}
			}
		}
		else if (qs.isCompleted() && !qs.isNowAvailable())
		{
			htmltext = "33357-03.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && (ArrayUtil.contains(MOBS, npc.getId())) && giveItemRandomly(killer, npc, LOYAL_SERVANS_BLOOD, 1, 10, 0.10, true))
		{
			qs.setCond(2, true);
		}
		
		if ((qs != null) && qs.isCond(2) && (ArrayUtil.contains(BOSSES, npc.getId())) && giveItemRandomly(killer, npc, TRUTTHFUL_ONES_BLOOD, 1, 10, 1, true))
		{
			qs.setCond(2, true);
		}
	}
}
