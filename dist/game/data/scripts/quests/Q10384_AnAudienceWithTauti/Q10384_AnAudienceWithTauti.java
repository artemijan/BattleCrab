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
package quests.Q10384_AnAudienceWithTauti;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;

import quests.Q10383_FergasonsOffer.Q10383_FergasonsOffer;

/**
 * @author hlwrave
 */
public class Q10384_AnAudienceWithTauti extends Quest
{
	// NPCs
	private static final int FERGASON = 33681;
	private static final int AKU = 33671;
	
	// Monsters
	private static final int TAUTI = 29237;
	
	// Items
	private static final int TAUTIS_FRAGMENT = 34960;
	private static final int BOTTLE_OF_TAUTIS_SOUL = 35295;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q10384_AnAudienceWithTauti()
	{
		super(10384);
		addStartNpc(FERGASON);
		addTalkId(FERGASON, AKU);
		addKillId(TAUTI);
		registerQuestItems(TAUTIS_FRAGMENT);
		addCondMinLevel(MIN_LEVEL, "maestro_ferguson_q10384_05.html");
		addCondCompletedQuest(Q10383_FergasonsOffer.class.getSimpleName(), "maestro_ferguson_q10384_06.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "maestro_ferguson_q10384_02.htm":
			case "maestro_ferguson_q10384_03.htm":
			case "maestro_ferguson_q10384_10.html":
			{
				htmltext = event;
				break;
			}
			case "maestro_ferguson_q10384_04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "sofa_aku_q10384_02.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "maestro_ferguson_q10384_11.html":
			{
				if (qs.getMemoState() < 1)
				{
					addExpAndSp(player, 951127800, 435041400);
					giveAdena(player, 3256740, true);
				}
				
				giveItems(player, BOTTLE_OF_TAUTIS_SOUL, 1);
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
			case FERGASON:
			{
				if (qs.isCreated())
				{
					htmltext = "maestro_ferguson_q10384_01.htm";
				}
				else if (qs.isStarted())
				{
					if (qs.isCond(1) || qs.isCond(2))
					{
						htmltext = "maestro_ferguson_q10384_08.html";
					}
					else if (qs.isCond(3) && hasQuestItems(player, TAUTIS_FRAGMENT))
					{
						htmltext = "maestro_ferguson_q10384_09.html";
					}
				}
				else if (qs.isCompleted())
				{
					htmltext = "maestro_ferguson_q10384_07.html";
				}
				break;
			}
			case AKU:
			{
				if (qs.isStarted())
				{
					htmltext = "sofa_aku_q10384_01.html";
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
		if ((qs != null) && qs.isCond(2))
		{
			qs.setCond(0);
			qs.setCond(3, true);
			giveItems(killer, TAUTIS_FRAGMENT, 1);
		}
	}
}
