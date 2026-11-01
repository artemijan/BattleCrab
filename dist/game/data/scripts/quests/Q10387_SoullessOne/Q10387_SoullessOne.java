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
package quests.Q10387_SoullessOne;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.enums.Movie;

import quests.Q10386_MysteriousJourney.Q10386_MysteriousJourney;

/**
 * Soulless One (10387)
 * @author St3eT
 */
public class Q10387_SoullessOne extends Quest
{
	// NPCs
	private static final int HASED = 33780;
	private static final int VERNA = 33796;
	private static final int SPICULA_ZERO = 25901;
	
	// Items
	private static final int EWR = 17526; // Scroll: Enchant Weapon (R-grade)
	private static final int EAR = 17527; // Scroll: Enchant Armor (R-grade)
	private static final int COKES = 36563; // Synthetic Cokes
	private static final int POUCH = 34861; // Ingredient and Hardener Pouch (R-grade)
	
	// Misc
	private static final int MIN_LEVEL = 93;
	
	public Q10387_SoullessOne()
	{
		super(10387);
		addStartNpc(HASED);
		addTalkId(HASED, VERNA);
		addKillId(SPICULA_ZERO);
		addCondMinLevel(MIN_LEVEL, "33796-12.htm");
		addCondCompletedQuest(Q10386_MysteriousJourney.class.getSimpleName(), "33796-12.htm");
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
			case "33796-02.html":
			case "33796-03.html":
			case "33796-04.html":
			case "33796-08.html":
			case "33796-09.html":
			{
				htmltext = event;
				break;
			}
			case "33780-02.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33796-05.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2, true);
					playMovie(player, Movie.SC_METUCELLAR_OPENING); // TODO: Need be created instance and run movie in instance
					htmltext = event;
				}
				break;
			}
			case "reward1":
			{
				if (qs.isCond(3))
				{
					giveItems(player, COKES, 68);
					giveItems(player, EAR, 5);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 817_330_500, 196_159);
					}
					
					qs.exitQuest(false, true);
					htmltext = "33796-10.html";
				}
				break;
			}
			case "reward2":
			{
				if (qs.isCond(3))
				{
					giveItems(player, EWR, 1);
					giveItems(player, EAR, 6);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 817_330_500, 196_159);
					}
					
					qs.exitQuest(false, true);
					htmltext = "33796-10.html";
				}
				break;
			}
			case "reward3":
			{
				if (qs.isCond(3))
				{
					giveItems(player, POUCH, 1);
					giveItems(player, EAR, 5);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 817_330_500, 196_159);
					}
					
					qs.exitQuest(false, true);
					htmltext = "33796-10.html";
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
				if (npc.getId() == HASED)
				{
					htmltext = "33780-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == HASED)
				{
					if (qs.isCond(1))
					{
						htmltext = "33780-03.html";
					}
				}
				else
				{
					switch (qs.getCond())
					{
						case 1:
						{
							htmltext = "33796-01.htm";
							break;
						}
						case 2:
						{
							htmltext = "33796-06.html";
							break;
						}
						case 3:
						{
							htmltext = "33796-07.html";
							break;
						}
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = npc.getId() == VERNA ? "33796-11.htm" : "33780-04.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(2))
		{
			qs.setCond(0);
			qs.setCond(3, true);
		}
	}
}
