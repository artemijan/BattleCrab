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
package quests.Q10704_BottleOfOctavisSoul;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10354_ResurrectedOwnerOfHall.Q10354_ResurrectedOwnerOfHall;

/**
 * Bottle of Octavis' Soul (10704)
 * @URL http://l2on.net/en/?c=quests&id=10704&game=1
 * @author Gigi
 */
public class Q10704_BottleOfOctavisSoul extends Quest
{
	// NPCs
	private static final int LYDIA = 32892;
	
	// Item
	private static final int OCTAVIS_SOUL_BOTTLE = 34884;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10704_BottleOfOctavisSoul()
	{
		super(10704);
		addStartNpc(LYDIA);
		addTalkId(LYDIA);
		addCondMinLevel(MIN_LEVEL, "32892-00.html");
		addCondCompletedQuest(Q10354_ResurrectedOwnerOfHall.class.getSimpleName(), "32892-00.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		final QuestState qs1 = player.getQuestState(Q10354_ResurrectedOwnerOfHall.class.getSimpleName());
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "32892-02.html":
			case "32892-03.html":
			case "32892-04.html":
			{
				htmltext = event;
				break;
			}
			case "32892-05.html":
			{
				qs.startQuest();
				break;
			}
			case "32892-06.html":
			{
				if (qs.isCond(1) && (getQuestItemsCount(player, OCTAVIS_SOUL_BOTTLE) >= 1))
				{
					takeItems(player, OCTAVIS_SOUL_BOTTLE, 1);
					qs1.setState(State.CREATED);
					qs1.setMemoState(1);
					qs.exitQuest(false, true);
					htmltext = event;
				}
				else
				{
					htmltext = getNoQuestMsg(player);
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
				if (getQuestItemsCount(player, OCTAVIS_SOUL_BOTTLE) >= 1)
				{
					htmltext = "32892-01.html";
				}
				else
				{
					htmltext = getNoQuestMsg(player);
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "32892-05.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getNoQuestMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
}
