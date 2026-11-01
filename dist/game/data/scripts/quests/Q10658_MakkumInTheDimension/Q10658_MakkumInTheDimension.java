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
package quests.Q10658_MakkumInTheDimension;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q00928_100DaySubjugationOperation.Q00928_100DaySubjugationOperation;

/**
 * Makkum in the Dimension (10658)
 * @URL https://l2wiki.com/Makkum_in_the_Dimension
 * @VIDEO https://www.youtube.com/watch?v=1z5zLnMmKtw
 * @author Gigi
 */
public class Q10658_MakkumInTheDimension extends Quest
{
	// Npc
	private static final int LIAS = 34265;
	
	// Items
	private static final int DIMENSIONAL_TRACES = 47511;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10658_MakkumInTheDimension()
	{
		super(10658);
		addStartNpc(LIAS);
		addTalkId(LIAS);
		addCondMinLevel(MIN_LEVEL, "34265-00.htm");
		addCondCompletedQuest(Q00928_100DaySubjugationOperation.class.getSimpleName(), "34265-00.htm");
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
			case "34265-02.htm":
			case "34265-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34265-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			default:
			{
				if (qs.isCond(2) && event.startsWith("giveReward_"))
				{
					final int itemId = Integer.parseInt(event.replace("giveReward_", ""));
					if (player.getLevel() >= MIN_LEVEL)
					{
						giveItems(player, itemId, 1);
						addExpAndSp(player, 4_303_647_428L, 10_328_753);
						takeItems(player, DIMENSIONAL_TRACES, -1);
						qs.exitQuest(QuestType.ONE_TIME, true);
						htmltext = "34265-07.html";
					}
					else
					{
						htmltext = getNoQuestLevelRewardMsg(player);
					}
				}
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
				if (getQuestItemsCount(player, DIMENSIONAL_TRACES) >= 100)
				{
					htmltext = "34265-01.htm";
					break;
				}
				
				htmltext = "34265-00.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.isCond(1)) ? "34265-05.html" : "34265-06.html";
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
}
