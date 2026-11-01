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
package quests.Q10372_PurgatoryVolvere;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10371_GraspThyPower.Q10371_GraspThyPower;

/**
 * Purgatory Volvere (10372)
 * @URL https://l2wiki.com/Purgatory_Volvere
 * @author Gigi
 */
public class Q10372_PurgatoryVolvere extends Quest
{
	// NPCs
	private static final int GERKENSHTEIN = 33648;
	private static final int ANDREI = 31292;
	
	// Monster's
	private static final int BLOODY_SUCCUBUS = 23185;
	
	// Items
	private static final int SUCCUBUS_ESENCE = 34766;
	private static final int GERKENSHTEINS_REPORT = 34767;
	
	// Reward
	private static final int EXP_REWARD = 23009000;
	private static final int SP_REWARD = 5522;
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 81;
	
	public Q10372_PurgatoryVolvere()
	{
		super(10372);
		addStartNpc(GERKENSHTEIN);
		addTalkId(GERKENSHTEIN, ANDREI);
		addKillId(BLOODY_SUCCUBUS);
		registerQuestItems(SUCCUBUS_ESENCE, GERKENSHTEINS_REPORT);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "no_level.html");
		addCondCompletedQuest(Q10371_GraspThyPower.class.getSimpleName(), "restriction.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "33648-02.htm":
			case "33648-03.htm":
			case "31292-02.html":
			case "31292-03.html":
			{
				htmltext = event;
				break;
			}
			case "33648-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33648-07.html":
			{
				takeItems(player, SUCCUBUS_ESENCE, -1);
				giveItems(player, GERKENSHTEINS_REPORT, 1);
				qs.setCond(0);
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			default:
			{
				if (event.startsWith("giveReward_") && qs.isCond(3))
				{
					final int itemId = Integer.parseInt(event.replace("giveReward_", ""));
					addExpAndSp(player, EXP_REWARD, SP_REWARD);
					takeItems(player, GERKENSHTEINS_REPORT, -1);
					giveItems(player, itemId, 15);
					qs.exitQuest(false, true);
					htmltext = "31292-04.html";
					break;
				}
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
			case State.COMPLETED:
			{
				htmltext = "complete.htm";
				break;
			}
			case State.CREATED:
			{
				if (npc.getId() == GERKENSHTEIN)
				{
					htmltext = (player.isInCategory(CategoryType.FOURTH_CLASS_GROUP) ? "33648-01.htm" : "complete.htm");
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case GERKENSHTEIN:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "33648-05.html";
								break;
							}
							case 2:
							{
								htmltext = "33648-06.html";
								break;
							}
							case 3:
							{
								htmltext = "33648-07.html";
								break;
							}
						}
						break;
					}
					case ANDREI:
					{
						if ((qs.isCond(3)) && (getQuestItemsCount(player, GERKENSHTEINS_REPORT) > 0))
						{
							htmltext = "31292-01.html";
						}
						break;
					}
				}
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
			if (giveItemRandomly(killer, npc, SUCCUBUS_ESENCE, 1, 10, 0.2, true))
			{
				qs.setCond(2, true);
			}
		}
	}
}
