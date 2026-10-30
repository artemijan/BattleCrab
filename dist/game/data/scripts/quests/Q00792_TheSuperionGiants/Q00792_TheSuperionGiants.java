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
package quests.Q00792_TheSuperionGiants;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10856_SuperionAppears.Q10856_SuperionAppears;

/**
 * The Superion Giants (792)
 * @URL https://l2wiki.com/The_Superion_Giants
 * @author Dmitri
 */
public class Q00792_TheSuperionGiants extends Quest
{
	// NPC
	private static final int HISTY = 34243;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23774, // Delta Bathus
		23775, // Kracos Delta
		23776, // Kshan Delta
		23777, // Royal temple
		23778, // Royal Shooter
		23779, // Royal wizard
		23780, // Royal First Temple
		23781, // Royal Accurate Shooter
		23782, // Royal Archmage
		23783, // Royal Gatekeeper
	};
	
	// Items
	private static final int TRACES_OF_THE_SCATTERED_GIANT = 47192;
	private static final int BASIC_SUPPLY_BOX = 48254;
	private static final int INTERMEDIATE_SUPPLY_BOX = 48255;
	private static final int ADVANCED_SUPPLY_BOX = 48256;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	public Q00792_TheSuperionGiants()
	{
		super(792);
		addStartNpc(HISTY);
		addTalkId(HISTY);
		addKillId(MONSTERS);
		registerQuestItems(TRACES_OF_THE_SCATTERED_GIANT);
		addCondMinLevel(MIN_LEVEL, "34243-00.htm");
		addCondCompletedQuest(Q10856_SuperionAppears.class.getSimpleName(), "34243-00.htm");
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
				if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 6) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 7))
				{
					htmltext = "34243-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 7)
				{
					htmltext = "34243-04b.htm";
					break;
				}
				
				htmltext = "34243-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 6) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 7))
				{
					htmltext = "34243-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 7)
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
						if ((getQuestItemsCount(player, TRACES_OF_THE_SCATTERED_GIANT) == 100) && (player.getLevel() >= MIN_LEVEL))
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
						if ((getQuestItemsCount(player, TRACES_OF_THE_SCATTERED_GIANT) == 200) && (player.getLevel() >= MIN_LEVEL))
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
						if ((getQuestItemsCount(player, TRACES_OF_THE_SCATTERED_GIANT) == 300) && (player.getLevel() >= MIN_LEVEL))
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
				// fallthrough?
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 6) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 7))
						{
							htmltext = "34243-04a.htm";
							break;
						}
						else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 7)
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
					if (giveItemRandomly(player, npc, TRACES_OF_THE_SCATTERED_GIANT, 1, 100, 1, true))
					{
						qs.setCond(5, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, TRACES_OF_THE_SCATTERED_GIANT, 1, 200, 1, true))
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 4:
				{
					if (giveItemRandomly(player, npc, TRACES_OF_THE_SCATTERED_GIANT, 1, 300, 1, true))
					{
						qs.setCond(7, true);
					}
					break;
				}
			}
		}
	}
}
