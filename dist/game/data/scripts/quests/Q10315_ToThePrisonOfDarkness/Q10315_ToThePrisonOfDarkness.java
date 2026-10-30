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
package quests.Q10315_ToThePrisonOfDarkness;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10307_TheCorruptedLeaderHisTruth.Q10307_TheCorruptedLeaderHisTruth;
import quests.Q10311_PeacefulDaysAreOver.Q10311_PeacefulDaysAreOver;

/**
 * To the Prison of Darkness (10315)
 * @URL https://l2wiki.com/To_the_Prison_of_Darkness
 * @author Gigi
 */
public class Q10315_ToThePrisonOfDarkness extends Quest
{
	// NPCs
	private static final int SLASKI = 32893;
	private static final int OPERA = 32946;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	// Item's
	private static final int EWR = 17526;
	
	public Q10315_ToThePrisonOfDarkness()
	{
		super(10315);
		addStartNpc(SLASKI);
		addTalkId(SLASKI, OPERA);
		addCondMinLevel(MIN_LEVEL, "32893-00.htm");
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
			case "32893-02.htm":
			case "32893-03.htm":
			case "32946-02.html":
			{
				htmltext = event;
				break;
			}
			case "32893-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32946-03.html":
			{
				giveAdena(player, 279513, false);
				giveItems(player, EWR, 1);
				addExpAndSp(player, 4038093, 969);
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
		final QuestState qs1 = player.getQuestState(Q10307_TheCorruptedLeaderHisTruth.class.getSimpleName());
		final QuestState qs2 = player.getQuestState(Q10311_PeacefulDaysAreOver.class.getSimpleName());
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == SLASKI)
				{
					if (((qs1 != null) && qs1.isCompleted()) || ((qs2 != null) && qs2.isCompleted()))
					{
						htmltext = "32893-01.htm";
						break;
					}
					
					htmltext = "32893-00.htm";
					break;
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case SLASKI:
					{
						if (qs.isCond(1))
						{
							htmltext = "32893-05.html";
						}
						break;
					}
					case OPERA:
					{
						if (qs.isCond(1))
						{
							htmltext = "32946-01.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				switch (npc.getId())
				{
					case SLASKI:
					{
						htmltext = "Complete.html";
						break;
					}
					case OPERA:
					{
						htmltext = "32946-00.html";
						break;
					}
				}
			}
		}
		
		return htmltext;
	}
}
