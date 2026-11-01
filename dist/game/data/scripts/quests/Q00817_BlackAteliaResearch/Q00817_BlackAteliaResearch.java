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
package quests.Q00817_BlackAteliaResearch;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10841_DeepInsideAteliaFortress.Q10841_DeepInsideAteliaFortress;

/**
 * Black Atelia Research (817)
 * @URL https://l2wiki.com/Black_Atelia_Research
 * @author Gigi
 */
public class Q00817_BlackAteliaResearch extends Quest
{
	// NPC
	private static final int KAYSYA = 34051;
	private static final int[] BOSS =
	{
		23603, // Guardian Sinistra
		23604, // Guardian Destra
		26128, // Kelbim's Clone
	};
	
	// Items
	private static final int BLACK_ATELIA_POWDER = 46145;
	private static final int HARDENER_POUCH_R = 32779;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	
	public Q00817_BlackAteliaResearch()
	{
		super(817);
		addStartNpc(KAYSYA);
		addTalkId(KAYSYA);
		addKillId(BOSS);
		registerQuestItems(BLACK_ATELIA_POWDER);
		addCondMinLevel(MIN_LEVEL, "34051-00.htm");
		addCondCompletedQuest(Q10841_DeepInsideAteliaFortress.class.getSimpleName(), "34051-00.htm");
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
			case "34051-02.htm":
			case "34051-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34051-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34051-07.html":
			{
				giveItems(player, HARDENER_POUCH_R, 1);
				addExpAndSp(player, 3631150845L, 8714700);
				qs.exitQuest(QuestType.REPEATABLE, true);
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
				htmltext = "34051-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34051-05.html";
				}
				else if (qs.isCond(2) && hasQuestItems(player, BLACK_ATELIA_POWDER))
				{
					htmltext = "34051-06.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 5, npc);
		if ((qs != null) && qs.isCond(1) && giveItemRandomly(killer, BLACK_ATELIA_POWDER, 1, 1, 0.6, true))
		{
			qs.setCond(2, true);
		}
	}
}
