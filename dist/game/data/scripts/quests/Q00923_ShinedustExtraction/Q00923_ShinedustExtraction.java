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
package quests.Q00923_ShinedustExtraction;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Shinedust Extraction (923)
 * @URL https://l2wiki.com/Shinedust_Extraction
 * @author Dmitri
 */
public class Q00923_ShinedustExtraction extends Quest
{
	// NPCs
	private static final int SHUMADRIBA = 34217;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23748, // Hero of the Younger Giants
		23733, // Junior Giant Warrior
		23734, // Junior Giant Mage
		23746, // Evolving Giant Warrior
		23747, // Mage of the Evolving Giants
		23739, // Mark
		23740, // Surien
		23741, // Berima
		23742, // Batus Nyei
		23743, // Krakos Nayi
		23744, // Kshana Nayi
		23745, // Lucus nyei
	};
	
	// Items
	private static final int SHINING_DUST = 46747;
	private static final int BASIC_SUPPLY_BOX = 47184;
	private static final int INTERMEDIATE_SUPPLY_BOX = 47185;
	private static final int ADVANCED_SUPPLY_BOX = 47186;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q00923_ShinedustExtraction()
	{
		super(923);
		addStartNpc(SHUMADRIBA);
		addTalkId(SHUMADRIBA);
		addKillId(MONSTERS);
		registerQuestItems(SHINING_DUST);
		addCondMinLevel(MIN_LEVEL, "34217-00.htm");
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
			case "34217-02.htm":
			case "34217-03.htm":
			case "34217-04.htm":
			case "34217-04a.htm":
			case "34217-04b.htm":
			case "34217-06.html":
			case "34217-06a.html":
			case "34217-06b.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 1) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 3))
				{
					htmltext = "34217-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 3)
				{
					htmltext = "34217-04b.htm";
					break;
				}
				
				htmltext = "34217-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 1) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 3))
				{
					htmltext = "34217-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 3)
				{
					htmltext = "34217-04b.htm";
					break;
				}
				
				htmltext = "34217-04.htm";
				break;
			}
			case "34217-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34217-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34217-07b.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34217-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 5:
					{
						if ((getQuestItemsCount(player, SHINING_DUST) == 200) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 14_831_100_000L, 14_831_100);
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
						if ((getQuestItemsCount(player, SHINING_DUST) == 400) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 29_662_200_000L, 29_662_200);
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
						if ((getQuestItemsCount(player, SHINING_DUST) == 600) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 44_493_300_000L, 44_493_300);
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
				htmltext = "34217-01.htm";
				// fallthrough?
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 1) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 3))
						{
							htmltext = "34217-04a.htm";
							break;
						}
						else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 3)
						{
							htmltext = "34217-04b.htm";
							break;
						}
						
						htmltext = "34217-04.htm";
						break;
					}
					case 2:
					{
						htmltext = "34217-08.html";
						break;
					}
					case 3:
					{
						htmltext = "34217-08a.html";
						break;
					}
					case 4:
					{
						htmltext = "34217-08b.html";
						break;
					}
					case 5:
					case 6:
					case 7:
					{
						htmltext = "34217-09.html";
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
					htmltext = "34217-01.htm";
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
					if (giveItemRandomly(player, npc, SHINING_DUST, 1, 200, 1, true))
					{
						qs.setCond(5, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, SHINING_DUST, 1, 400, 1, true))
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 4:
				{
					if (giveItemRandomly(player, npc, SHINING_DUST, 1, 600, 1, true))
					{
						qs.setCond(7, true);
					}
					break;
				}
			}
		}
	}
}
