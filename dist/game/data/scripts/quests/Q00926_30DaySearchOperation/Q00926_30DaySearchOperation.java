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
package quests.Q00926_30DaySearchOperation;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Exploring the Dimension - 30-day Search Operation (926)
 * @URL https://l2wiki.com/Exploring_the_Dimension_-_30-day_Search_Operation
 * @Custom-Made based on quest 928
 * @author Mobius
 */
public class Q00926_30DaySearchOperation extends Quest
{
	// NPC
	private static final int BELOA = 34227;
	
	// Monsters
	private static final int WANDERING_OF_DIMENSION = 23755;
	private static final int LOST_SOUL_DIMENSION = 23757;
	private static final int ROAMING_VENGEANCE = 23759;
	
	// Items
	private static final int SPIRIT_FRAGMENTS = 46785;
	private static final int BELOAS_SUPPLY_ITEMS = 47043;
	private static final int REMNANT_OF_THE_RIFT = 46787;
	
	// Misc
	private static final QuestType QUEST_TYPE = QuestType.DAILY; // REPEATABLE, ONE_TIME, DAILY
	private static final int MIN_LEVEL = 95;
	private static final int MAX_LEVEL = 102;
	
	public Q00926_30DaySearchOperation()
	{
		super(926);
		addStartNpc(BELOA);
		addTalkId(BELOA);
		addKillId(WANDERING_OF_DIMENSION, LOST_SOUL_DIMENSION, ROAMING_VENGEANCE);
		registerQuestItems(SPIRIT_FRAGMENTS);
		addCondMinLevel(MIN_LEVEL, "34227-00.html");
		addCondMaxLevel(MAX_LEVEL, getNoQuestMsg(null));
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		switch (event)
		{
			case "34227-02.htm":
			case "34227-03.htm":
			{
				return event;
			}
			case "34227-04.htm":
			{
				qs.startQuest();
				break;
			}
			case "34227-07.html":
			{
				if (player.getLevel() >= MIN_LEVEL)
				{
					if (getQuestItemsCount(player, REMNANT_OF_THE_RIFT) < 29)
					{
						giveItems(player, REMNANT_OF_THE_RIFT, 1);
						giveItems(player, BELOAS_SUPPLY_ITEMS, 1);
						addExpAndSp(player, 1507592779L, 3618222);
						qs.exitQuest(QUEST_TYPE, true);
						break;
					}
					
					addExpAndSp(player, 1507592779L, 3618222);
					giveItems(player, REMNANT_OF_THE_RIFT, 1);
					giveItems(player, BELOAS_SUPPLY_ITEMS, 1);
					qs.exitQuest(QUEST_TYPE, true);
				}
				break;
			}
			default:
			{
				return null;
			}
		}
		
		return event;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = getAlreadyCompletedMsg(player, QUEST_TYPE);
					break;
				}
				
				qs.setState(State.CREATED);
				// fallthrough
			}
			case State.CREATED:
			{
				htmltext = "34227-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.isCond(1)) ? "34227-05.html" : "34227-06.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && (qs.isCond(1)))
		{
			switch (npc.getId())
			{
				case WANDERING_OF_DIMENSION:
				case LOST_SOUL_DIMENSION:
				case ROAMING_VENGEANCE:
				{
					if (giveItemRandomly(killer, npc, SPIRIT_FRAGMENTS, 1, 100, 1, true))
					{
						qs.setCond(2, true);
					}
					break;
				}
			}
		}
	}
}
