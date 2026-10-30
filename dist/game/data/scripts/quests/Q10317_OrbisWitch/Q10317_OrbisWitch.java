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
package quests.Q10317_OrbisWitch;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;

import quests.Q10316_UndecayingMemoryOfThePast.Q10316_UndecayingMemoryOfThePast;

/**
 * Orbis' Witch (10317)
 * @URL https://l2wiki.com/Orbis%27_Witch
 * @author Gigi
 */
public class Q10317_OrbisWitch extends Quest
{
	// NPCs
	private static final int OPERA = 32946;
	private static final int LYDIA = 32892;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10317_OrbisWitch()
	{
		super(10317);
		addStartNpc(OPERA);
		addTalkId(OPERA, LYDIA);
		addCondMinLevel(MIN_LEVEL, "32946-09.html");
		addCondCompletedQuest(Q10316_UndecayingMemoryOfThePast.class.getSimpleName(), "32946-09.html");
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
			case "32946-02.html":
			case "32946-03.html":
			case "32946-04.html":
			case "32946-05.htm":
			{
				htmltext = event;
				break;
			}
			case "32946-06.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32892-02.html":
			{
				giveAdena(player, 506760, false);
				addExpAndSp(player, 7412805, 1779);
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
			case OPERA:
			{
				if (qs.isCreated())
				{
					htmltext = "32946-01.htm";
				}
				else if (qs.isCond(1))
				{
					htmltext = "32946-07.html";
				}
				else if (qs.isCompleted())
				{
					htmltext = "32946-08.html";
				}
				break;
			}
			case LYDIA:
			{
				if (qs.isCond(1))
				{
					htmltext = "32892-01.html";
					break;
				}
			}
		}
		
		return htmltext;
	}
}
