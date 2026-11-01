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
package quests.Q10817_ExaltedOneWhoOvercomesTheLimit;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10811_ExaltedOneWhoFacesTheLimit.Q10811_ExaltedOneWhoFacesTheLimit;

/**
 * Exalted, One Who Overcomes the Limit (10817)
 * @URL https://l2wiki.com/Exalted,_One_Who_Overcomes_the_Limit
 * @author Mobius
 */
public class Q10817_ExaltedOneWhoOvercomesTheLimit extends Quest
{
	// NPC
	private static final int LIONEL = 33907;
	
	// Items
	private static final int DAICHIR_SERTIFICATE = 45628;
	private static final int OLYMPIAD_MANAGER_CERTIFICATE = 45629;
	private static final int ISHUMA_CERTIFICATE = 45630;
	private static final int SIR_KRISTOF_RODEMAI_CERTIFICATE = 45631;
	private static final int LIONEL_MISSION_LIST_2 = 45632;
	
	// Rewards
	private static final int SPELLBOOK_DIGNITY_OF_THE_EXALTED = 45923;
	private static final int SPELLBOOK_BELIEF_OF_THE_EXALTED = 45925;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	private static final int MIN_COMPLETE_LEVEL = 100;
	
	public Q10817_ExaltedOneWhoOvercomesTheLimit()
	{
		super(10817);
		addStartNpc(LIONEL);
		addTalkId(LIONEL);
		addCondMinLevel(MIN_LEVEL, "33907-07.html");
		addCondCompletedQuest(Q10811_ExaltedOneWhoFacesTheLimit.class.getSimpleName(), "33907-02.html");
		registerQuestItems(LIONEL_MISSION_LIST_2, DAICHIR_SERTIFICATE, OLYMPIAD_MANAGER_CERTIFICATE, ISHUMA_CERTIFICATE, SIR_KRISTOF_RODEMAI_CERTIFICATE);
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
			case "33907-03.htm":
			case "33907-04.htm":
			{
				htmltext = event;
				break;
			}
			case "33907-05.html":
			{
				if (qs.isCreated())
				{
					giveItems(player, LIONEL_MISSION_LIST_2, 1);
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "33907-09.html":
			{
				if (hasQuestItems(player, DAICHIR_SERTIFICATE, OLYMPIAD_MANAGER_CERTIFICATE, ISHUMA_CERTIFICATE, SIR_KRISTOF_RODEMAI_CERTIFICATE) && (player.getLevel() >= MIN_COMPLETE_LEVEL))
				{
					giveItems(player, SPELLBOOK_DIGNITY_OF_THE_EXALTED, 1);
					giveItems(player, SPELLBOOK_BELIEF_OF_THE_EXALTED, 1);
					qs.exitQuest(false, true);
					htmltext = event;
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
				htmltext = "33907-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (hasQuestItems(player, DAICHIR_SERTIFICATE, OLYMPIAD_MANAGER_CERTIFICATE, ISHUMA_CERTIFICATE, SIR_KRISTOF_RODEMAI_CERTIFICATE) && (player.getLevel() >= MIN_COMPLETE_LEVEL))
				{
					htmltext = "33907-08.html";
				}
				else
				{
					htmltext = "33907-06.html";
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
}
