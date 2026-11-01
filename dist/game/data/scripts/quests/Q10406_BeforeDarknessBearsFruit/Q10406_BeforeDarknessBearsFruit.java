/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package quests.Q10406_BeforeDarknessBearsFruit;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10405_KartiasSeed.Q10405_KartiasSeed;

/**
 * Before Darkness Bears Fruit (10406)
 * @author St3eT
 */
public class Q10406_BeforeDarknessBearsFruit extends Quest
{
	// NPCs
	private static final int SHUVANN = 33867;
	private static final int KARTIAS_FLOWER = 19470;
	
	// Misc
	private static final int MIN_LEVEL = 61;
	private static final int MAX_LEVEL = 65;
	
	public Q10406_BeforeDarknessBearsFruit()
	{
		super(10406);
		addStartNpc(SHUVANN);
		addTalkId(SHUVANN);
		addKillId(KARTIAS_FLOWER);
		addCondNotRace(Race.ERTHEIA, "33867-08.html");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33867-09.htm");
		addCondCompletedQuest(Q10405_KartiasSeed.class.getSimpleName(), "33867-09.htm");
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
			case "33867-02.htm":
			case "33867-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33867-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33867-07.html":
			{
				if (qs.isCond(2))
				{
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 13561681, 750);
					}
					
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
				htmltext = "33867-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = qs.isCond(1) ? "33867-05.html" : "33867-06.html";
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
		if ((qs != null) && qs.isStarted() && qs.isCond(1))
		{
			int killCount = qs.getInt("KILLED_COUNT");
			if (killCount < 10)
			{
				killCount++;
				qs.set("KILLED_COUNT", killCount);
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			
			if (killCount == 10)
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isStarted() && qs.isCond(1))
		{
			final Set<NpcLogListHolder> npcLogList = new HashSet<>(1);
			npcLogList.add(new NpcLogListHolder(KARTIAS_FLOWER, false, qs.getInt("KILLED_COUNT")));
			return npcLogList;
		}
		
		return super.getNpcLogList(player);
	}
}
