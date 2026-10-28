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
package quests.Q00585_CantGoAgainstTheTime;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Can't Go Against the Time (585)
 * @URL https://l2wiki.com/Can%27t_Go_Against_the_Time
 * @author Dmitri
 */
public class Q00585_CantGoAgainstTheTime extends Quest
{
	// NPC
	private static final int FAIRY_CITIZEN = 32921;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		22866, // Fairy Warrior (Imperfect)
		22874, // Fairy Rogue (Imperfect)
		22882, // Fairy Knight (Imperfect)
		22890, // Satyr Wizard (Imperfect)
		22898, // Satyr Summoner (Imperfect)
		22906, // Satyr Witch (Imperfect)
		22865, // Fairy Warrior (Mature)
		22873, // Fairy Rogue (Mature)
		22881, // Fairy Knight (Mature)
		22889, // Satyr Wizard (Mature)
		22897, // Satyr Summoner (Mature)
		22905, // Satyr Witch (Mature)
		22864, // Fairy Warrior (Wicked)
		22872, // Fairy Rogue (Wicked)
		22880, // Fairy Knight (Wicked)
		22888, // Satyr Wizard (Wicked)
		22896, // Satyr Summoner (Wicked)
		22904, // Satyr Witch (Wicked)
		19400 // Cocoon Destroyer
	};
	
	// Items
	private static final int TRACES_OF_MUTATION = 48381;
	
	// Misc
	private static final int MIN_LEVEL = 88;
	private static final int MAX_LEVEL = 98;
	
	public Q00585_CantGoAgainstTheTime()
	{
		super(585);
		addStartNpc(FAIRY_CITIZEN);
		addTalkId(FAIRY_CITIZEN);
		addKillId(MONSTERS);
		registerQuestItems(TRACES_OF_MUTATION);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "nolevel.html");
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
			case "32921-02.htm":
			case "32921-03.htm":
			{
				htmltext = event;
				break;
			}
			case "32921-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32921-07.html":
			{
				giveAdena(player, 536520, true);
				addExpAndSp(player, 429526470, 429510);
				qs.exitQuest(QuestType.DAILY, true);
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
				htmltext = "32921-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "32921-05.html";
				}
				else
				{
					htmltext = "32921-06.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					qs.setState(State.CREATED);
					htmltext = "32921-01.htm";
				}
				else
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && giveItemRandomly(killer, npc, TRACES_OF_MUTATION, 1, 100, 1, true))
		{
			qs.setCond(2, true);
		}
	}
}
