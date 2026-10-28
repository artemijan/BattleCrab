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
package quests.Q00831_SayhasScheme;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Sayha's Scheme (831)
 * @URL https://l2wiki.com/Sayha%27s_Scheme
 * @author Liamxroy
 */
public class Q00831_SayhasScheme extends Quest
{
	// NPC
	private static final int YUYURIA = 34100;
	private static final int YUYURIA_FINISH = 34155;
	private static final int ALTAR = 34103;
	
	// Items
	private static final int DESTROYED_MARK_FRAGMENT = 46374;
	private static final int GLUDIN_HERO_REWARD = 46375;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q00831_SayhasScheme()
	{
		super(831);
		addStartNpc(YUYURIA);
		addTalkId(YUYURIA, YUYURIA_FINISH);
		addKillId(ALTAR);
		addCondMinLevel(MIN_LEVEL, "34100-00.htm");
		registerQuestItems(DESTROYED_MARK_FRAGMENT);
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
			case "34100-02.htm":
			case "34100-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34100-04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34155-02.html":
			{
				if (qs.isCond(2))
				{
					takeItems(player, -1, DESTROYED_MARK_FRAGMENT);
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
				if (npc.getId() == YUYURIA)
				{
					htmltext = "34100-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == YUYURIA)
				{
					htmltext = "34100-05.html";
				}
				else
				{
					htmltext = "34155-01.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable() && (npc.getId() == YUYURIA))
				{
					qs.setState(State.CREATED);
					htmltext = "34100-01.htm";
				}
				else
				{
					htmltext = "34100-06.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
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
			final QuestState qs = getQuestState(member, false);
			if ((qs != null) && qs.isCond(1) && member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE) && giveItemRandomly(member, npc, DESTROYED_MARK_FRAGMENT, 1, 10, 1, true))
			{
				qs.setCond(2, true);
			}
		}
	}
}
