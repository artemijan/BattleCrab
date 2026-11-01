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
package quests.Q00737_ASwordHiddenInASmile;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * A Sword Hidden in a Smile (737)
 * @URL https://l2wiki.com/A_Sword_Hidden_in_a_Smile
 * @author Dmitri
 */
public class Q00737_ASwordHiddenInASmile extends Quest
{
	// NPCs
	private static final int HISTY = 34243;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23816, // Batus Ohm
		23817, // Kshana Oma
	};
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	// Items
	private static final int FIGHTER_STONE_SHARD = 48166;
	private static final int PROOF_OF_LIBERTY = 48165;
	private static final int BASIC_SUPPLY_BOX = 48254;
	private static final int INTERMEDIATE_SUPPLY_BOX = 48255;
	private static final int ADVANCED_SUPPLY_BOX = 48256;
	
	public Q00737_ASwordHiddenInASmile()
	{
		super(737);
		addStartNpc(HISTY);
		addTalkId(HISTY);
		addKillId(MONSTERS);
		registerQuestItems(PROOF_OF_LIBERTY);
		addCondMinLevel(MIN_LEVEL, "34243-00.htm");
		addFactionLevel(Faction.GIANT_TRACKERS, 4, "34243-00.htm");
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
			case "34243-02.htm":
			case "34243-03.htm":
			case "34243-04.htm":
			case "34243-04a.htm":
			case "34243-04b.htm":
			case "34243-06.html":
			case "34243-06a.html":
			case "34243-06b.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 8) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 9))
				{
					htmltext = "34243-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 9)
				{
					htmltext = "34243-04b.htm";
					break;
				}
				
				htmltext = "34243-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 8) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 9))
				{
					htmltext = "34243-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 9)
				{
					htmltext = "34243-04b.htm";
					break;
				}
				
				htmltext = "34243-04.htm";
				break;
			}
			case "34243-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34243-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34243-07b.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34243-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 5:
					{
						if ((getQuestItemsCount(player, PROOF_OF_LIBERTY) == 10) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 22_221_427_950L, 22_221_360);
							giveItems(player, FIGHTER_STONE_SHARD, 1);
							addFactionPoints(player, Faction.GIANT_TRACKERS, 100);
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
						if ((getQuestItemsCount(player, PROOF_OF_LIBERTY) == 20) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 44_442_855_900L, 44_442_720);
							giveItems(player, FIGHTER_STONE_SHARD, 3);
							addFactionPoints(player, Faction.GIANT_TRACKERS, 200);
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
						if ((getQuestItemsCount(player, PROOF_OF_LIBERTY) == 30) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 66_664_283_850L, 66_664_080);
							giveItems(player, FIGHTER_STONE_SHARD, 5);
							addFactionPoints(player, Faction.GIANT_TRACKERS, 300);
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
				htmltext = "34243-01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 8) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 9))
						{
							htmltext = "34243-04a.htm";
							break;
						}
						else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 9)
						{
							htmltext = "34243-04b.htm";
							break;
						}
						
						htmltext = "34243-04.htm";
						break;
					}
					case 2:
					{
						htmltext = "34243-08.html";
						break;
					}
					case 3:
					{
						htmltext = "34243-08a.html";
						break;
					}
					case 4:
					{
						htmltext = "34243-08b.html";
						break;
					}
					case 5:
					case 6:
					case 7:
					{
						htmltext = "34243-09.html";
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
					htmltext = "34243-01.htm";
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
					if (giveItemRandomly(player, npc, PROOF_OF_LIBERTY, 1, 10, 1, true))
					{
						qs.setCond(5, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, PROOF_OF_LIBERTY, 1, 20, 1, true))
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 4:
				{
					if (giveItemRandomly(player, npc, PROOF_OF_LIBERTY, 1, 30, 1, true))
					{
						qs.setCond(7, true);
					}
					break;
				}
			}
		}
	}
}
