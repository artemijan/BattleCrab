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
package quests.Q00845_SabotageTheEmbryoSupplies;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10844_BloodyBattleSeizingSupplies.Q10844_BloodyBattleSeizingSupplies;

/**
 * Sabotage the Embryo Supplies (845)
 * @URL https://l2wiki.com/Sabotage_the_Embryo_Supplies
 * @author Dmitri
 */
public class Q00845_SabotageTheEmbryoSupplies extends Quest
{
	// NPCs
	private static final int LOGART_VAN_DYKE = 34235;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23589, //
		23507, //
		23506, //
		23505, //
		23508, //
		23537, //
		23538, //
		23509, //
		23512, //
		23511 //
	};
	
	// Items
	private static final int EMBRYO_SUPPLIES = 47197;
	
	// Rewards
	private static final int BASIC_SUPPLY_BOX = 47175; // Kingdom's Royal Guard Basic Supply Box Atelia Fortress
	private static final int INTERMEDIATE_SUPPLY_BOX = 47176; // Kingdom's Royal Guard Intermediate Supply Box Atelia Fortress
	private static final int ADVANCED_SUPPLY_BOX = 47177; // Kingdom's Royal Guard Advanced Supply Box Atelia Fortress
	
	// Misc
	private static final int MIN_LEVEL = 101;
	
	public Q00845_SabotageTheEmbryoSupplies()
	{
		super(845);
		addStartNpc(LOGART_VAN_DYKE);
		addTalkId(LOGART_VAN_DYKE);
		addKillId(MONSTERS);
		registerQuestItems(EMBRYO_SUPPLIES);
		addCondMinLevel(MIN_LEVEL, "34235-00.htm");
		addCondCompletedQuest(Q10844_BloodyBattleSeizingSupplies.class.getSimpleName(), "34235-00.htm");
		addFactionLevel(Faction.KINGDOM_ROYAL_GUARDS, 2, "34235-00.htm");
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
			case "34235-02.htm":
			case "34235-03.htm":
			case "34235-04.htm":
			case "34235-04a.htm":
			case "34235-04b.htm":
			case "34235-06.html":
			case "34235-06a.html":
			case "34235-06b.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 5) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 8))
				{
					htmltext = "34235-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 8)
				{
					htmltext = "34235-04b.htm";
					break;
				}
				
				htmltext = "34235-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 5) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 8))
				{
					htmltext = "34235-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 8)
				{
					htmltext = "34235-04b.htm";
					break;
				}
				
				htmltext = "34235-04.htm";
				break;
			}
			case "34235-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34235-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34235-07b.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34235-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 5:
					{
						if ((getQuestItemsCount(player, EMBRYO_SUPPLIES) == 40) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 18155754360L, 18155700);
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
					case 6:
					{
						if ((getQuestItemsCount(player, EMBRYO_SUPPLIES) == 80) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 36311508720L, 36311400);
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
					case 7:
					{
						if ((getQuestItemsCount(player, EMBRYO_SUPPLIES) == 120) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 54467263080L, 54467100);
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
				htmltext = "34235-01.htm";
				// fallthrough?
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 5) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 8))
						{
							htmltext = "34235-04a.htm";
							break;
						}
						else if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 8)
						{
							htmltext = "34235-04b.htm";
							break;
						}
						
						htmltext = "34235-04.htm";
						break;
					}
					case 2:
					{
						htmltext = "34235-08.html";
						break;
					}
					case 3:
					{
						htmltext = "34235-08a.html";
						break;
					}
					case 4:
					{
						htmltext = "34235-08b.html";
						break;
					}
					case 5:
					case 6:
					case 7:
					{
						htmltext = "34235-09.html";
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
					htmltext = "34235-01.htm";
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
					if (giveItemRandomly(player, npc, EMBRYO_SUPPLIES, 1, 40, 0.5, true))
					{
						qs.setCond(5, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, EMBRYO_SUPPLIES, 1, 80, 0.5, true))
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 4:
				{
					if (giveItemRandomly(player, npc, EMBRYO_SUPPLIES, 1, 120, 0.5, true))
					{
						qs.setCond(7, true);
					}
					break;
				}
			}
		}
	}
}
