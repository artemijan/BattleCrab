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
package quests.Q10830_TheLostGardenOfSpirits;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10829_InSearchOfTheCause.Q10829_InSearchOfTheCause;

/**
 * The Lost Garden of Spirits (10830)
 * @URL https://l2wiki.com/The_Lost_Garden_of_Spirits
 * @author Gigi
 */
public class Q10830_TheLostGardenOfSpirits extends Quest
{
	// NPC
	private static final int CYPHONIA = 34055;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23550, // Kerberos Lager
		23551, // Kerberos Fort
		23552, // Kerberos Nero
		23553, // Fury Sylph Barrena
		23555, // Fury Sylph Temptress
		23556, // Fury Sylph Purka
		23557, // Fury Kerberos Leger
		23558 // Fury Kerberos Nero
	};
	
	// Items
	private static final int UNSTABLE_SPIRITS_ENERGY = 45821;
	private static final int SOE = 46158;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10830_TheLostGardenOfSpirits()
	{
		super(10830);
		addStartNpc(CYPHONIA);
		addTalkId(CYPHONIA);
		addKillId(MONSTERS);
		registerQuestItems(UNSTABLE_SPIRITS_ENERGY);
		addCondMinLevel(MIN_LEVEL, "34055-00.htm");
		addCondCompletedQuest(Q10829_InSearchOfTheCause.class.getSimpleName(), "34055-00.htm");
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
			case "34055-02.htm":
			case "34055-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34055-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34055-07.html":
			{
				giveItems(player, SOE, 1);
				addExpAndSp(player, 1637472704L, 14237820);
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
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "34055-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34055-05.html";
				}
				else
				{
					htmltext = "34055-06.html";
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
		if ((qs != null) && qs.isCond(1) && giveItemRandomly(killer, npc, UNSTABLE_SPIRITS_ENERGY, 1, 100, 0.5, true))
		{
			qs.setCond(2, true);
		}
	}
}
