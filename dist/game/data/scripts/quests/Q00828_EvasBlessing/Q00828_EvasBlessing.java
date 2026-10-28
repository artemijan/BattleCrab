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
package quests.Q00828_EvasBlessing;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

/**
 * Eva's Blessing (828)
 * @URL https://l2wiki.com/Eva%27s_Blessing
 * @author Liamxroy
 */
public class Q00828_EvasBlessing extends Quest
{
	// NPC
	private static final int ADONIUS = 34097;
	private static final int ADONIUS_FINISH = 34152;
	private static final int[] CAPTIVES =
	{
		34104,
		34105,
		34106,
		34107,
		34108,
		34109,
		34110,
		34111,
		34112,
		34113,
		34114,
		34115,
		34116,
		34117,
		34118,
		34119,
		34120,
		34121,
		34122,
		34123,
		34124,
		34125,
		34126,
		34127,
		34128,
		34129,
		34130,
		34131,
		34132,
		34133,
		34134,
		34135,
	};
	
	// Items
	private static final int GLUDIN_HERO_REWARD = 46375;
	
	// Misc
	private static final NpcStringId[] CAPTIVES_TEXT =
	{
		NpcStringId.WHAT_WHO_ARE_YOU,
		NpcStringId.WE_MUST_ALERT_THE_COMMANDER_ABOUT_THESE_INTRUDERS,
		NpcStringId.ALERT_EVERYONE,
	};
	private static final int MIN_LEVEL = 100;
	
	public Q00828_EvasBlessing()
	{
		super(828);
		addStartNpc(ADONIUS);
		addFirstTalkId(CAPTIVES);
		addTalkId(ADONIUS, ADONIUS_FINISH);
		addCondMinLevel(MIN_LEVEL, "34097-00.htm");
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
			case "34097-02.htm":
			case "34097-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34097-04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34152-02.html":
			{
				if (qs.isCond(2))
				{
					rewardItems(player, GLUDIN_HERO_REWARD, 1);
					addExpAndSp(player, 2422697985L, 5814450);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
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
				if (npc.getId() == ADONIUS)
				{
					htmltext = "34097-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == ADONIUS)
				{
					htmltext = "34097-05.html";
				}
				else
				{
					htmltext = "34152-01.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable() && (npc.getId() == ADONIUS))
				{
					qs.setState(State.CREATED);
					htmltext = "34097-01.htm";
				}
				else
				{
					htmltext = "34097-06.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.isCond(1)))
		{
			List<Player> members = new ArrayList<>();
			if (player.getParty() != null)
			{
				members = player.getParty().getMembers();
			}
			else
			{
				members.add(player);
			}
			
			for (Player member : members)
			{
				final QuestState ms = getQuestState(member, false);
				if ((ms != null) && ms.isCond(1))
				{
					int count = ms.getMemoState();
					count++;
					if (count < 20)
					{
						ms.setMemoState(count);
						final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
						log.addNpcString(NpcStringId.RESCUING_CAPTIVES, count);
						member.sendPacket(log);
					}
					
					if (count >= 20)
					{
						ms.setCond(2, true);
					}
				}
			}
			
			npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(CAPTIVES_TEXT));
			npc.deleteMe();
			return "captive-0" + getRandom(1, 3) + ".html";
		}
		
		return null;
	}
}
