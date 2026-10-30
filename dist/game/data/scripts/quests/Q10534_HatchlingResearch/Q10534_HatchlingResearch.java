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
package quests.Q10534_HatchlingResearch;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Hatchling Research (10534)
 * @URL https://l2wiki.com/Hatchling_Research
 * @author Gigi
 * @date 2017-09-04 - [12:49:15]
 */
public class Q10534_HatchlingResearch extends Quest
{
	// NPC
	private static final int STENA = 34221;
	private static final int DRAGON_HATCHING = 23434;
	private static final int LEOPARD_DRAGON = 23435;
	
	// Item
	private static final int HATCHLING_FLASH = 46735;
	
	// Misc
	private static final int MIN_LEVEL = 81;
	private static final int MAX_LEVEL = 84;
	
	public Q10534_HatchlingResearch()
	{
		super(10534);
		addStartNpc(STENA);
		addTalkId(STENA);
		addKillId(DRAGON_HATCHING, LEOPARD_DRAGON);
		registerQuestItems(HATCHLING_FLASH);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "34221-08.htm");
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
			case "34221-02.htm":
			case "34221-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34221-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34221-07.html":
			{
				if (qs.isCond(2))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 362_053_391, 19_840);
						qs.exitQuest(QuestType.ONE_TIME, true);
						htmltext = event;
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
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "34221-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34221-05.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "34221-06.html";
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
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && (qs.isCond(1)))
		{
			if (giveItemRandomly(killer, npc, HATCHLING_FLASH, 1, 50, 0.7, true))
			{
				qs.setCond(2, true);
			}
		}
	}
}
