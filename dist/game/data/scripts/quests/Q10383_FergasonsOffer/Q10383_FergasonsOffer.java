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
package quests.Q10383_FergasonsOffer;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;

import quests.Q10381_ToTheSeedOfHellfire.Q10381_ToTheSeedOfHellfire;

/**
 * @author hlwrave
 */
public class Q10383_FergasonsOffer extends Quest
{
	// NPCs
	private static final int SIZRAK = 33669;
	private static final int AKU = 33671;
	private static final int FERGASON = 33681;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23213,
		23214,
		23215,
		23216,
		23217,
		23218,
		23219
	};
	
	// Item
	private static final int UNSTABLE_PETRA = 34958;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q10383_FergasonsOffer()
	{
		super(10383);
		addStartNpc(SIZRAK);
		addTalkId(SIZRAK, AKU, FERGASON);
		addKillId(MONSTERS);
		registerQuestItems(UNSTABLE_PETRA);
		addCondMinLevel(MIN_LEVEL, "sofa_sizraku_q10383_04.html");
		addCondCompletedQuest(Q10381_ToTheSeedOfHellfire.class.getSimpleName(), "sofa_sizraku_q10383_07.html");
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
			case "sofa_sizraku_q10383_02.htm":
			case "maestro_ferguson_q10383_02.html":
			case "maestro_ferguson_q10383_03.html":
			{
				htmltext = event;
				break;
			}
			case "sofa_sizraku_q10383_03.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "maestro_ferguson_q10383_04.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "sofa_aku_q10383_03.html":
			{
				takeItems(player, UNSTABLE_PETRA, -1L);
				addExpAndSp(player, 951127800, 435041400);
				giveAdena(player, 3256740, true);
				qs.exitQuest(QuestType.ONE_TIME, true);
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
			case SIZRAK:
			{
				if (qs.isCreated())
				{
					htmltext = "sofa_sizraku_q10383_01.htm";
				}
				else if (qs.isStarted())
				{
					htmltext = "sofa_sizraku_q10383_06.html";
				}
				else if (qs.isCompleted())
				{
					htmltext = "sofa_sizraku_q10383_05.html";
				}
				break;
			}
			case FERGASON:
			{
				if (qs.isCond(1))
				{
					htmltext = "maestro_ferguson_q10383_01.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "maestro_ferguson_q10383_05.html";
				}
				break;
			}
			case AKU:
			{
				if (qs.isCond(1))
				{
					htmltext = "sofa_aku_q10383_01.html";
				}
				else if (qs.isCond(3))
				{
					htmltext = "sofa_aku_q10383_02.html";
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
		if ((qs != null) && qs.isCond(2) && qs.isStarted() && giveItemRandomly(killer, npc, UNSTABLE_PETRA, 1, 20, 0.75, true))
		{
			qs.setCond(0);
			qs.setCond(3, true);
		}
	}
}
