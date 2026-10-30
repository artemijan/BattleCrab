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
package quests.Q10821_HelpingOthers;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10817_ExaltedOneWhoOvercomesTheLimit.Q10817_ExaltedOneWhoOvercomesTheLimit;

/**
 * Helping Others (10821)
 * @URL https://l2wiki.com/Helping_Others
 * @author Mobius
 */
public class Q10821_HelpingOthers extends Quest
{
	// NPC
	private static final int SIR_ERIC_RODEMAI = 30868;
	
	// Items
	private static final int MENTEE_MARK = 33804;
	private static final int DAICHIR_SERTIFICATE = 45628;
	private static final int OLYMPIAD_MANAGER_CERTIFICATE = 45629;
	private static final int ISHUMA_CERTIFICATE = 45630;
	
	// Rewards
	private static final int SIR_KRISTOF_RODEMAI_CERTIFICATE = 45631;
	private static final int SPELLBOOK_FAVOR_OF_THE_EXALTED = 45928;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	public Q10821_HelpingOthers()
	{
		super(10821);
		addStartNpc(SIR_ERIC_RODEMAI);
		addTalkId(SIR_ERIC_RODEMAI);
		addCondMinLevel(MIN_LEVEL, "30868-02.html");
		addCondStartedQuest(Q10817_ExaltedOneWhoOvercomesTheLimit.class.getSimpleName(), "30868-03.html");
		// registerQuestItems(MENTEE_MARK); Should they be removed when abandoning quest?
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
			case "30868-04.htm":
			case "30868-05.htm":
			{
				htmltext = event;
				break;
			}
			case "30868-06.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30868-09.html":
			{
				if (qs.isCond(1) && (getQuestItemsCount(player, MENTEE_MARK) >= 45000))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						if (hasQuestItems(player, DAICHIR_SERTIFICATE, ISHUMA_CERTIFICATE, OLYMPIAD_MANAGER_CERTIFICATE))
						{
							htmltext = "30868-10.html";
						}
						else
						{
							htmltext = event;
						}
						
						takeItems(player, MENTEE_MARK, 45000);
						giveItems(player, SIR_KRISTOF_RODEMAI_CERTIFICATE, 1);
						giveItems(player, SPELLBOOK_FAVOR_OF_THE_EXALTED, 1);
						qs.exitQuest(false, true);
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
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "30868-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (getQuestItemsCount(player, MENTEE_MARK) >= 45000)
				{
					htmltext = "30868-08.html";
				}
				else
				{
					htmltext = "30868-07.html";
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
