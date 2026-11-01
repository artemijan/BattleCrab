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
package quests.Q00816_PlansToRepairTheStronghold;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Plans to Repair the Stronghold (816)
 * @URL https://l2wiki.com/Plans_to_Repair_the_Stronghold
 * @author Dmitri
 */
public class Q00816_PlansToRepairTheStronghold extends Quest
{
	// NPCs
	private static final int ADOLPH = 34058;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23505, // Fortress Raider 101
		23506, // Fortress Guardian Captain 101
		23507, // Atelia Passionate Soldier 101
		23508, // Atelia Elite Captain 101
		23509, // Fortress Dark Wizard 102
		23510, // Atelia Flame Master 102
		23511, // Fortress Archon 102
		23512 // Atelia High Priest 102
	};
	
	// Items
	private static final int MATERIAL_QUEST = 46142; // Stronghold Flag Repair Supplies
	private static final int BASIC_SUPPLY_BOX = 47175;
	private static final int INTERMEDIATE_SUPPLY_BOX = 47176;
	private static final int ADVANCED_SUPPLY_BOX = 47177;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	
	public Q00816_PlansToRepairTheStronghold()
	{
		super(816);
		addStartNpc(ADOLPH);
		addTalkId(ADOLPH);
		addKillId(MONSTERS);
		registerQuestItems(MATERIAL_QUEST);
		addCondMinLevel(MIN_LEVEL, "34058-00.htm");
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
			case "34058-02.htm":
			case "34058-03.htm":
			case "34058-04.htm":
			case "34058-04a.htm":
			case "34058-04b.htm":
			case "34058-04d.htm":
			case "34058-06.html":
			case "34058-06a.html":
			case "34058-06b.html":
			case "34058-06d.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 1) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 3))
				{
					htmltext = "34058-04a.htm";
					break;
				}
				else if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 3) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 6))
				{
					htmltext = "34058-04b.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 6)
				{
					htmltext = "34058-04d.htm";
					break;
				}
				
				htmltext = "34058-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 1) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 3))
				{
					htmltext = "34058-04a.htm";
					break;
				}
				else if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 3) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 6))
				{
					htmltext = "34058-04b.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 6)
				{
					htmltext = "34058-04d.htm";
					break;
				}
				
				htmltext = "34058-04.htm";
				break;
			}
			case "34058-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34058-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34058-07b.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34058-07d.html":
			{
				qs.setCond(5, true);
				htmltext = event;
				break;
			}
			case "34058-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 6:
					{
						if ((getQuestItemsCount(player, MATERIAL_QUEST) == 200) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 18_155_754_360L, 18_155_700);
							addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 100);
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
						if ((getQuestItemsCount(player, MATERIAL_QUEST) == 400) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 36_311_508_720L, 36_311_400);
							addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 200);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
					case 8:
					{
						if ((getQuestItemsCount(player, MATERIAL_QUEST) == 600) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 54_467_263_080L, 54_467_100);
							addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 300);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
					case 9:
					{
						if ((getQuestItemsCount(player, MATERIAL_QUEST) == 800) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 50)
							{
								giveItems(player, BASIC_SUPPLY_BOX, 2);
							}
							else if (chance < 50)
							{
								giveItems(player, INTERMEDIATE_SUPPLY_BOX, 2);
							}
							else if (chance < 50)
							{
								giveItems(player, ADVANCED_SUPPLY_BOX, 2);
							}
							
							addExpAndSp(player, 72_623_017_440L, 72_622_800);
							addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 400);
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
				htmltext = "34058-01.htm";
				// fallthrough
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 1) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 3))
						{
							htmltext = "34058-04a.htm";
							break;
						}
						else if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 3) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 6))
						{
							htmltext = "34058-04b.htm";
							break;
						}
						else if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 6)
						{
							htmltext = "34058-04d.htm";
							break;
						}
						
						htmltext = "34058-04.htm";
						break;
					}
					case 2:
					{
						htmltext = "34058-08.html";
						break;
					}
					case 3:
					{
						htmltext = "34058-08a.html";
						break;
					}
					case 4:
					{
						htmltext = "34058-08b.html";
						break;
					}
					case 5:
					{
						htmltext = "34058-08d.html";
						break;
					}
					case 6:
					case 7:
					case 8:
					case 9:
					{
						htmltext = "34058-09.html";
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
					htmltext = "34058-01.htm";
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
					if (giveItemRandomly(player, npc, MATERIAL_QUEST, 1, 200, 1, true))
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, MATERIAL_QUEST, 1, 400, 1, true))
					{
						qs.setCond(7, true);
					}
					break;
				}
				case 4:
				{
					if (giveItemRandomly(player, npc, MATERIAL_QUEST, 1, 600, 1, true))
					{
						qs.setCond(8, true);
					}
					break;
				}
				case 5:
				{
					if (giveItemRandomly(player, npc, MATERIAL_QUEST, 1, 800, 1, true))
					{
						qs.setCond(9, true);
					}
					break;
				}
			}
		}
	}
}
