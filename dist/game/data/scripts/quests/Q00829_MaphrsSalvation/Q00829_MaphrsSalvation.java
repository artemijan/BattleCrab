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
package quests.Q00829_MaphrsSalvation;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Maphr's Salvation (829)
 * @URL https://l2wiki.com/Maphr%27s_Salvation
 * @author Liamxroy
 */
public class Q00829_MaphrsSalvation extends Quest
{
	// NPC
	private static final int BLACKSMITH_KLUTO = 34098;
	private static final int BLACKSMITH_KLUTO_FINISH = 34153;
	private static final int TRANSPORT_GOODS_NPC = 34102;
	
	// Items
	private static final int TRASPORT_GOODS_ITEM = 46373;
	private static final int GLUDIN_HERO_REWARD = 46375;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q00829_MaphrsSalvation()
	{
		super(829);
		addStartNpc(BLACKSMITH_KLUTO);
		addFirstTalkId(TRANSPORT_GOODS_NPC);
		addTalkId(BLACKSMITH_KLUTO, BLACKSMITH_KLUTO_FINISH);
		addCondMinLevel(MIN_LEVEL, "34098-00.htm");
		registerQuestItems(TRASPORT_GOODS_ITEM);
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
			case "34098-02.htm":
			case "34098-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34098-04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34153-02.html":
			{
				if (qs.isCond(2))
				{
					takeItems(player, -1, TRASPORT_GOODS_ITEM);
					rewardItems(player, GLUDIN_HERO_REWARD, 1);
					addExpAndSp(player, 2175228000L, 5220534);
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
				if (npc.getId() == BLACKSMITH_KLUTO)
				{
					htmltext = "34098-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == BLACKSMITH_KLUTO)
				{
					htmltext = "34098-05.html";
				}
				else
				{
					htmltext = "34153-01.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable() && (npc.getId() == BLACKSMITH_KLUTO))
				{
					qs.setState(State.CREATED);
					htmltext = "34098-01.htm";
				}
				else
				{
					htmltext = "34098-06.html";
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
					if (getQuestItemsCount(member, TRASPORT_GOODS_ITEM) < 1)
					{
						giveItems(member, TRASPORT_GOODS_ITEM, 1);
					}
					
					if (getQuestItemsCount(member, TRASPORT_GOODS_ITEM) >= 1)
					{
						ms.setCond(2, true);
					}
				}
			}
			
			npc.deleteMe();
			return "34102-01.html";
		}
		
		return null;
	}
}
