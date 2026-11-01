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
package quests.Q00844_GiantsTreasure;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Giant's Treasure (844)
 * @URL https://l2wiki.com/Giant%27s_Treasure
 * @author Dmitri
 */
public class Q00844_GiantsTreasure extends Quest
{
	// NPCs
	private static final int KRENAHT = 34237;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23730, // Om Bathus
		23731, // Om Carcass
		23732, // Om Kshana
		23751 // Om Lucas
	};
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	// Items
	private static final int OLD_BOX = 47212; // Quest item: Old Box
	
	// Reward
	private static final int GIANT_TRACKERS_BASIC_SUPPLY_BOX = 47359;
	private static final int GIANT_TRACKERS_INTERMEDIATE_SUPPLY_BOX = 47360;
	private static final int GIANT_TRACKERS_ADVANCED_SUPPLY_BOX = 47361;
	
	public Q00844_GiantsTreasure()
	{
		super(844);
		addStartNpc(KRENAHT);
		addTalkId(KRENAHT);
		addKillId(MONSTERS);
		registerQuestItems(OLD_BOX);
		addCondMinLevel(MIN_LEVEL, "34237-00.htm");
		addFactionLevel(Faction.GIANT_TRACKERS, 2, "34237-00.htm");
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
			case "34237-02.htm":
			case "34237-03.htm":
			case "34237-04.htm":
			case "34237-04a.htm":
			case "34237-06.html":
			case "34237-06a.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 4)
				{
					htmltext = "34237-04a.htm";
					break;
				}
				
				htmltext = "34237-04.htm";
				break;
			}
			case "return":
			{
				if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 4)
				{
					htmltext = "34237-04a.htm";
					break;
				}
				
				htmltext = "34237-04.htm";
				break;
			}
			case "34237-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34237-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34237-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 4:
					{
						if ((getQuestItemsCount(player, OLD_BOX) == 10) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 2)
							{
								giveItems(player, GIANT_TRACKERS_ADVANCED_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, GIANT_TRACKERS_INTERMEDIATE_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, GIANT_TRACKERS_BASIC_SUPPLY_BOX, 1);
							}
							
							addExpAndSp(player, 5_932_440_000L, 14_237_820);
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
					case 5:
					{
						if ((getQuestItemsCount(player, OLD_BOX) == 20) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 2)
							{
								giveItems(player, GIANT_TRACKERS_ADVANCED_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, GIANT_TRACKERS_BASIC_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, GIANT_TRACKERS_INTERMEDIATE_SUPPLY_BOX, 1);
							}
							
							addExpAndSp(player, 11_864_880_000L, 28_475_640);
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
				htmltext = "34237-01.htm";
				// fallthrou
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 4)
						{
							htmltext = "34237-04a.htm";
							break;
						}
						
						htmltext = "34237-04.htm";
						break;
					}
					case 2:
					{
						htmltext = "34237-08.html";
						break;
					}
					case 3:
					{
						htmltext = "34237-08a.html";
						break;
					}
					case 4:
					case 5:
					{
						htmltext = "34237-09.html";
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
					htmltext = "34237-01.htm";
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
					if (giveItemRandomly(player, npc, OLD_BOX, 1, 10, 1, true))
					{
						qs.setCond(4, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, OLD_BOX, 1, 20, 1, true))
					{
						qs.setCond(5, true);
					}
					break;
				}
			}
		}
	}
}
