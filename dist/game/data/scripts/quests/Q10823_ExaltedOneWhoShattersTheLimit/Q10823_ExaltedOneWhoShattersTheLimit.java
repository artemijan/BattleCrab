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
package quests.Q10823_ExaltedOneWhoShattersTheLimit;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10817_ExaltedOneWhoOvercomesTheLimit.Q10817_ExaltedOneWhoOvercomesTheLimit;

/**
 * Exalted, One Who Shatters the Limit (10823)
 * @URL https://l2wiki.com/Exalted,_One_Who_Shatters_the_Limit
 * @author Mobius
 */
public class Q10823_ExaltedOneWhoShattersTheLimit extends Quest
{
	// NPC
	private static final int LIONEL = 33907;
	
	// Items
	private static final int MERLOT_CERTIFICATE = 46056;
	private static final int KURTIZ_CERTIFICATE = 46057;
	private static final int MAMMON_CERTIFICATE = 45635;
	private static final int GUSTAV_CERTIFICATE = 45636;
	private static final int LIONEL_MISSION_LIST_3 = 45637;
	
	// Rewards
	private static final int EXALTED_CLOAK = 37763;
	private static final int OBTAIN_EXALTED_STATUS = 45638;
	private static final int EXALTED_TIARA = 45644;
	private static final int DIGNITY_OF_THE_EXALTED = 45924;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	private static final int MIN_DUALCLASS_LEVEL = 100;
	
	public Q10823_ExaltedOneWhoShattersTheLimit()
	{
		super(10823);
		addStartNpc(LIONEL);
		addTalkId(LIONEL);
		addCondMinLevel(MIN_LEVEL, "33907-07.html");
		addCondCompletedQuest(Q10817_ExaltedOneWhoOvercomesTheLimit.class.getSimpleName(), "33907-02.html");
		registerQuestItems(LIONEL_MISSION_LIST_3, MERLOT_CERTIFICATE, KURTIZ_CERTIFICATE, MAMMON_CERTIFICATE, GUSTAV_CERTIFICATE);
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
					giveItems(player, LIONEL_MISSION_LIST_3, 1);
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "33907-09.html":
			{
				if (hasQuestItems(player, MERLOT_CERTIFICATE, KURTIZ_CERTIFICATE, MAMMON_CERTIFICATE, GUSTAV_CERTIFICATE) && ((player.getDualClass() != null) && (player.getDualClass().getLevel() >= MIN_DUALCLASS_LEVEL)))
				{
					giveItems(player, EXALTED_CLOAK, 1);
					giveItems(player, OBTAIN_EXALTED_STATUS, 1);
					giveItems(player, EXALTED_TIARA, 1);
					giveItems(player, DIGNITY_OF_THE_EXALTED, 1);
					
					// Give Exalted status here?
					// https://l2wiki.com/Noblesse
					player.setNobleLevel(2);
					player.broadcastUserInfo();
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
				if (hasQuestItems(player, MERLOT_CERTIFICATE, KURTIZ_CERTIFICATE, MAMMON_CERTIFICATE, GUSTAV_CERTIFICATE) && ((player.getDualClass() != null) && (player.getDualClass().getLevel() >= MIN_DUALCLASS_LEVEL)))
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
