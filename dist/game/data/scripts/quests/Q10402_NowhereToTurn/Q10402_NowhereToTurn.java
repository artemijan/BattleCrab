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
package quests.Q10402_NowhereToTurn;

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
import org.l2jmobius.gameserver.network.NpcStringId;

/**
 * Nowhere to Turn (10402)
 * @author St3eT
 */
public class Q10402_NowhereToTurn extends Quest
{
	// NPCs
	private static final int EBLUNE = 33865;
	private static final int[] MONSTERS =
	{
		20679, // Marsh Stalker
		20680, // Marsh Drake
		21017, // Fallen Orc
		21018, // Ancient Gargoyle
		21019, // Fallen Orc Archer
		21020, // Fallen Orc Shaman
		21021, // Sharp Talon Tiger
		21022, // Fallen Orc Captain
	};
	
	// Misc
	private static final int MIN_LEVEL = 56;
	private static final int MAX_LEVEL = 61;
	
	public Q10402_NowhereToTurn()
	{
		super(10402);
		addStartNpc(EBLUNE);
		addTalkId(EBLUNE);
		addKillId(MONSTERS);
		addCondNotRace(Race.ERTHEIA, "33865-08.html");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33865-09.htm");
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
			case "33865-02.htm":
			case "33865-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33865-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33865-07.html":
			{
				if (qs.isCond(2))
				{
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 9115741, 1315);
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
				htmltext = "33865-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = qs.isCond(1) ? "33865-05.html" : "33865-06.html";
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
			if (killCount < 100)
			{
				killCount++;
				qs.set("KILLED_COUNT", killCount);
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			
			if (killCount == 100)
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
			npcLogList.add(new NpcLogListHolder(NpcStringId.ELIMINATE_MONSTERS_IN_THE_FORSAKEN_PLAINS, qs.getInt("KILLED_COUNT")));
			return npcLogList;
		}
		
		return super.getNpcLogList(player);
	}
}
