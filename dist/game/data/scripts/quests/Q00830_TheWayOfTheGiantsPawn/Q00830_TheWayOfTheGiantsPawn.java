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
package quests.Q00830_TheWayOfTheGiantsPawn;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

/**
 * The Way of the Giant's Pawn (830)
 * @URL https://l2wiki.com/The_Way_of_the_Giant%27s_Pawn
 * @author Liamxroy
 */
public class Q00830_TheWayOfTheGiantsPawn extends Quest
{
	// NPC
	private static final int YENICHE = 34099;
	private static final int YENICHE_FINISH = 34154;
	private static final int[] UNIT_ELITE_SOLDIER =
	{
		23616, // Unit 1 Elite Soldier
		23617, // Unit 2 Elite Soldier
		23618, // Unit 3 Elite Soldier
		23619, // Unit 4 Elite Soldier
		23620, // Unit 5 Elite Soldier
		23621, // Unit 6 Elite Soldier
		23622, // Unit 7 Elite Soldier
		23623, // Unit 8 Elite Soldier
		23624, // Unit 1 Elite Soldier
		23625, // Unit 2 Elite Soldier
		23626, // Unit 3 Elite Soldier
		23627, // Unit 4 Elite Soldier
		23628, // Unit 5 Elite Soldier
		23629, // Unit 6 Elite Soldier
		23630, // Unit 7 Elite Soldier
		23631, // Unit 8 Elite Soldier
		23632, // Unit 1 Elite Soldier
		23633, // Unit 2 Elite Soldier
		23634, // Unit 3 Elite Soldier
		23635, // Unit 4 Elite Soldier
		23636, // Unit 5 Elite Soldier
		23637, // Unit 6 Elite Soldier
		23638, // Unit 7 Elite Soldier
		23639, // Unit 8 Elite Soldier
		23640, // Unit 1 Elite Soldier
		23641, // Unit 2 Elite Soldier
		23642, // Unit 3 Elite Soldier
		23643, // Unit 4 Elite Soldier
		23644, // Unit 5 Elite Soldier
		23645, // Unit 6 Elite Soldier
		23646, // Unit 7 Elite Soldier
		23647, // Unit 8 Elite Soldier
	};
	
	// Items
	private static final int GLUDIN_HERO_REWARD = 46375;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q00830_TheWayOfTheGiantsPawn()
	{
		super(830);
		addStartNpc(YENICHE);
		addTalkId(YENICHE, YENICHE_FINISH);
		addKillId(UNIT_ELITE_SOLDIER);
		addCondMinLevel(MIN_LEVEL, "34099-00.htm");
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
			case "34099-02.htm":
			case "34099-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34099-04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34154-02.html":
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
				if (npc.getId() == YENICHE)
				{
					htmltext = "34099-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == YENICHE)
				{
					htmltext = "34099-05.html";
				}
				else
				{
					htmltext = "34154-01.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable() && (npc.getId() == YENICHE))
				{
					qs.setState(State.CREATED);
					htmltext = "34099-01.htm";
				}
				else
				{
					htmltext = "34099-06.html";
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
			if ((qs != null) && qs.isCond(1) && member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
			{
				int count = qs.getMemoState();
				count++;
				if (count < 45)
				{
					qs.setMemoState(count);
					final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
					log.addNpcString(NpcStringId.DEFEAT_THE_ELITE_SOLDIERS_OF_THE_REVOLUTIONARIES, count);
					member.sendPacket(log);
				}
				
				if (count >= 45)
				{
					qs.setCond(2, true);
				}
			}
		}
	}
}
