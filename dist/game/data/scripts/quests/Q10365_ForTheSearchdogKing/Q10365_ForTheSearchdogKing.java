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
package quests.Q10365_ForTheSearchdogKing;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.Q10364_ObligationsOfTheSeeker.Q10364_ObligationsOfTheSeeker;

/**
 * For the Searchdog King (10365)
 * @author Stayway
 */
public class Q10365_ForTheSearchdogKing extends Quest
{
	// NPCs
	private static final int DEP = 33453;
	private static final int SEBION = 32978;
	
	// MOBs
	private static final int EYESAROCH = 23122;
	private static final int CRITTER = 22993;
	private static final int RIDER = 22995;
	
	// Items
	private static final int KINGS_TONIC = 47607;
	
	// Misc
	private static final int MIN_LEVEL = 15;
	private static final int MAX_LEVEL = 25;
	
	public Q10365_ForTheSearchdogKing()
	{
		super(10365);
		addStartNpc(DEP);
		addTalkId(DEP, SEBION);
		registerQuestItems(KINGS_TONIC);
		addKillId(EYESAROCH, CRITTER, RIDER);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33453-06.html");
		addCondCompletedQuest(Q10364_ObligationsOfTheSeeker.class.getSimpleName(), "33453-06.html");
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
			case "33453-02.htm":
			case "33453-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33453-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32978-02.html":
			{
				if (qs.isCond(2))
				{
					addExpAndSp(player, 172000, 15);
					qs.exitQuest(false, true);
					htmltext = event;
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
		if ((qs != null) && qs.isCond(1) && giveItemRandomly(killer, npc, KINGS_TONIC, 1, 20, 0.5, true))
		{
			qs.setCond(0);
			qs.setCond(2);
			showOnScreenMsg(killer, NpcStringId.USE_THE_YE_SAGIRA_TELEPORT_DEVICE_SHINING_WITH_A_RED_SHIMMER_TO_GO_TO_EXPLORATION_AREA_5, ExShowScreenMessage.TOP_CENTER, 4500);
		}
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
				if (npc.getId() == DEP)
				{
					htmltext = "33453-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case DEP:
					{
						if (qs.isCond(1))
						{
							showOnScreenMsg(player, NpcStringId.USE_THE_YE_SAGIRA_TELEPORT_DEVICE_SHINING_WITH_A_RED_SHIMMER_TO_GO_TO_EXPLORATION_AREA_5, ExShowScreenMessage.TOP_CENTER, 10000);
							htmltext = "33453-05.html";
						}
						break;
					}
					case SEBION:
					{
						if (qs.isCond(2))
						{
							htmltext = "32978-01.html";
						}
						break;
					}
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
