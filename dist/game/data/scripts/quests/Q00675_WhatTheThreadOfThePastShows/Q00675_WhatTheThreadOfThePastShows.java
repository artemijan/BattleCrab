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
package quests.Q00675_WhatTheThreadOfThePastShows;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * What the Thread of the Past Shows (675)
 * @URL https://l2wiki.com/What_the_Thread_of_the_Past_Shows
 * @author Dmitri
 */
public class Q00675_WhatTheThreadOfThePastShows extends Quest
{
	// NPCs
	private static final int CERIEL = 34415;
	private static final int NERUPA = 34412;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		24118, // Crystal Reep
		24120, // Crystal Needle
		24122, // Treant Blossom
		24124, // Flush Teasle
		24126 // Creeper Rampike
	};
	
	// Items
	private static final int NERUPA_BUD_PETAL = 48368;
	private static final int BASIC_SUPPLY_BOX = 48397; // Mother Tree Guardians Basic Treasure Chest: Shadow of the Mother Tree
	private static final int INTERMEDIATE_SUPPLY_BOX = 48398; // Mother Tree Guardians Intermediate Treasure Chest: Shadow of the Mother Tree
	private static final int ADVANCED_SUPPLY_BOX = 48399; // Mother Tree Guardians Advanced Treasure Chest: Shadow of the Mother Tree
	
	// Misc
	private static final int MIN_LEVEL = 103;
	
	public Q00675_WhatTheThreadOfThePastShows()
	{
		super(675);
		addStartNpc(CERIEL);
		addTalkId(CERIEL, NERUPA);
		addKillId(MONSTERS);
		registerQuestItems(NERUPA_BUD_PETAL);
		addCondMinLevel(MIN_LEVEL, "34415-00.htm");
		addFactionLevel(Faction.MOTHER_TREE_GUARDIANS, 4, "34415-00.htm");
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
			case "34415-02.htm":
			case "34415-03.htm":
			case "34415-04.htm":
			case "34415-04a.htm":
			case "34415-04b.htm":
			case "34415-06.html":
			case "34415-06a.html":
			case "34415-06b.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 8) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 9))
				{
					htmltext = "34415-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 9)
				{
					htmltext = "34415-04b.htm";
					break;
				}
				
				htmltext = "34415-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 8) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 9))
				{
					htmltext = "34415-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 9)
				{
					htmltext = "34415-04b.htm";
					break;
				}
				
				htmltext = "34415-04.htm";
				break;
			}
			case "34415-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34415-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34415-07b.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34412-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 5:
					{
						if ((getQuestItemsCount(player, NERUPA_BUD_PETAL) == 30) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 2)
							{
								giveItems(player, ADVANCED_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, INTERMEDIATE_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, BASIC_SUPPLY_BOX, 1);
							}
							
							giveAdena(player, 2495659, true);
							addExpAndSp(player, 27_191_624_760L, 27_191_610);
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, 100);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
					case 6:
					{
						if ((getQuestItemsCount(player, NERUPA_BUD_PETAL) == 60) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 2)
							{
								giveItems(player, ADVANCED_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, BASIC_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, INTERMEDIATE_SUPPLY_BOX, 1);
							}
							
							giveAdena(player, 4991318, true);
							addExpAndSp(player, 54_383_249_520L, 54_383_220);
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, 200);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
					case 7:
					{
						if ((getQuestItemsCount(player, NERUPA_BUD_PETAL) == 90) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 2)
							{
								giveItems(player, BASIC_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, INTERMEDIATE_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, ADVANCED_SUPPLY_BOX, 1);
							}
							
							giveAdena(player, 7486978, true);
							addExpAndSp(player, 81_574_874_280L, 81_574_830);
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, 300);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
				}
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
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == CERIEL)
				{
					htmltext = "34415-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case CERIEL:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 8) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 9))
								{
									htmltext = "34415-04a.htm";
									break;
								}
								else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 9)
								{
									htmltext = "34415-04b.htm";
									break;
								}
								
								htmltext = "34415-04.htm";
								break;
							}
							case 2:
							{
								htmltext = "34415-08.html";
								break;
							}
							case 3:
							{
								htmltext = "34415-08a.html";
								break;
							}
							case 4:
							{
								htmltext = "34415-08b.html";
								break;
							}
						}
						break;
					}
					case NERUPA:
					{
						switch (qs.getCond())
						{
							case 5:
							case 6:
							case 7:
							{
								htmltext = "34412-09.html";
								break;
							}
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				else
				{
					qs.setState(State.CREATED);
					htmltext = "34415-01.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		executeForEachPlayer(player, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.getCond() > 1) && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			switch (qs.getCond())
			{
				case 2:
				{
					if (giveItemRandomly(player, npc, NERUPA_BUD_PETAL, 1, 30, 1, true))
					{
						qs.setCond(5, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, NERUPA_BUD_PETAL, 1, 60, 1, true))
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 4:
				{
					if (giveItemRandomly(player, npc, NERUPA_BUD_PETAL, 1, 90, 1, true))
					{
						qs.setCond(7, true);
					}
					break;
				}
			}
		}
	}
}
