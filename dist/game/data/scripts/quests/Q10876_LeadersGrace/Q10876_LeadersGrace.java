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
package quests.Q10876_LeadersGrace;

import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10873_ExaltedReachingAnotherLevel.Q10873_ExaltedReachingAnotherLevel;

/**
 * Leader's Grace (10876)
 * @URL https://l2wiki.com/Leader%27s_Grace
 * @author Dmitri
 */
public class Q10876_LeadersGrace extends Quest
{
	// NPC
	private static final int SANTIAGO = 34138;
	
	// Items
	private static final int SPECIAL_FISH_STEW = 47887;
	private static final int R_GRADE_GEMSTONE = 19440;
	private static final ItemHolder LIONEL_HUNTERS_LIST_PART_4 = new ItemHolder(47829, 1);
	
	// Rewards
	private static final int SANTIAGO_CERTIFICATE = 47832;
	private static final int ELCYUM_CRYSTAL = 36514;
	
	// Misc
	private static final int MIN_LEVEL = 103;
	
	public Q10876_LeadersGrace()
	{
		super(10876);
		addStartNpc(SANTIAGO);
		addTalkId(SANTIAGO);
		addCondMinLevel(MIN_LEVEL, "34138-00.htm");
		addCondStartedQuest(Q10873_ExaltedReachingAnotherLevel.class.getSimpleName(), "34138-00.htm");
		registerQuestItems(SPECIAL_FISH_STEW, R_GRADE_GEMSTONE);
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
			case "34138-02.htm":
			case "34138-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34138-04.html":
			{
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_4))
				{
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "34138-07.html":
			{
				if ((getQuestItemsCount(player, SPECIAL_FISH_STEW) >= 50) && (getQuestItemsCount(player, R_GRADE_GEMSTONE) >= 80))
				{
					takeItems(player, SPECIAL_FISH_STEW, -50);
					takeItems(player, R_GRADE_GEMSTONE, -80);
					giveItems(player, ELCYUM_CRYSTAL, 1);
					giveItems(player, SANTIAGO_CERTIFICATE, 1);
					qs.exitQuest(false, true);
					
					final Quest mainQ = ScriptManager.getInstance().getScript(Q10873_ExaltedReachingAnotherLevel.class.getSimpleName());
					if (mainQ != null)
					{
						mainQ.notifyEvent("SUBQUEST_FINISHED_NOTIFY", npc, player);
					}
					
					htmltext = event;
				}
				else
				{
					htmltext = getNoQuestLevelRewardMsg(player);
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
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_4))
				{
					htmltext = "34138-01.htm";
				}
				else
				{
					htmltext = "34138-00.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					if ((getQuestItemsCount(player, SPECIAL_FISH_STEW) >= 50) && (getQuestItemsCount(player, R_GRADE_GEMSTONE) >= 80))
					{
						htmltext = "34138-06.html";
					}
					else
					{
						htmltext = "34138-05.html";
					}
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
}
