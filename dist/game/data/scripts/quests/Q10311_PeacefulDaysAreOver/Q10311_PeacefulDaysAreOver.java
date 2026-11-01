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
package quests.Q10311_PeacefulDaysAreOver;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;

import quests.Q10312_AbandonedGodsCreature.Q10312_AbandonedGodsCreature;

/**
 * Peaceful Days are Over (10311)
 * @URL https://l2wiki.com/Peaceful_Days_are_Over
 * @author Gigi
 */
public class Q10311_PeacefulDaysAreOver extends Quest
{
	// npc
	private static final int SELINA = 33032;
	private static final int SLASKI = 32893;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	public Q10311_PeacefulDaysAreOver()
	{
		super(10311);
		addStartNpc(SELINA);
		addTalkId(SELINA, SLASKI);
		addCondMinLevel(MIN_LEVEL, "33032-00.htm");
		addCondCompletedQuest(Q10312_AbandonedGodsCreature.class.getSimpleName(), "33032-00.htm");
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
			case "33032-02.htm":
			case "33032-03.htm":
			case "32893-02.html":
			case "32893-03.html":
			case "32893-04.html":
			{
				htmltext = event;
				break;
			}
			case "33032-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32893-05.html":
			{
				giveAdena(player, 489220, false);
				addExpAndSp(player, 7168395, 1720);
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
		switch (npc.getId())
		{
			case SELINA:
			{
				if (qs.isCreated())
				{
					htmltext = "33032-01.htm";
				}
				else if (qs.isCond(1))
				{
					htmltext = "33032-05.html";
				}
				else if (qs.isCompleted())
				{
					htmltext = "Complete.html";
				}
				break;
			}
			case SLASKI:
			{
				if (qs.isCond(1))
				{
					htmltext = "32893-01.html";
				}
				else if (qs.isCompleted())
				{
					htmltext = "32893-00.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
}
