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
package quests.Q00491_InNominePatris;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * In Nomine Patris (491)
 * @URL https://l2wiki.com/In_Nomine_Patris
 * @author Gigi
 */
public class Q00491_InNominePatris extends Quest
{
	// NPCs
	private static final int SIRIK = 33649;
	
	// Monster's
	private static final int[] MONSTERS =
	{
		23181, // Succubus Soldier
		23182, // Succubus Warrior
		23183, // Succubus Archer
		23184 // Succubus Shaman
	};
	
	// Items
	private static final int DIMENSIONAL_FRAGMENT = 34768;
	
	// Others
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 81;
	
	// Reward
	private static final int EXP_REWARD = 184210;
	private static final int SP_REWARD = 45;
	
	public Q00491_InNominePatris()
	{
		super(491);
		addStartNpc(SIRIK);
		addTalkId(SIRIK);
		addKillId(MONSTERS);
		registerQuestItems(DIMENSIONAL_FRAGMENT);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "no_level.html");
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
			case "33649-02.htm":
			case "33649-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33649-04.htm":
			{
				qs.startQuest();
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, SIRIK, NpcStringId.HURRY_AND_DEFEAT_THOSE_MONSTERS));
				htmltext = event;
				break;
			}
			case "33649-06.html":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, SIRIK, NpcStringId.HMM_THANK_YOU_SO_THEN_MAYBE_I_WON_T_HAVE_TO_GET_INVOLVED));
				addExpAndSp(player, (EXP_REWARD * player.getLevel()), (SP_REWARD * player.getLevel()));
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
		if (npc.getId() == SIRIK)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "complete.htm";
						break;
					}
					
					qs.setState(State.CREATED);
				}
				case State.CREATED:
				{
					htmltext = (player.isAwakenedClass() ? "33649-01.htm" : "33649-00.html");
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "33649-07.html";
					}
					else if (qs.isCond(2))
					{
						htmltext = "33649-05.html";
					}
					break;
				}
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
			if (giveItemRandomly(killer, npc, DIMENSIONAL_FRAGMENT, 1, 50, 0.5, true))
			{
				qs.setCond(2, true);
			}
		}
	}
}
