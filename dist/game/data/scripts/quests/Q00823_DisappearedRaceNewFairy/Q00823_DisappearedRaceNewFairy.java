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
package quests.Q00823_DisappearedRaceNewFairy;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Disappeared Race, New Fairy (823)
 * @URL https://l2wiki.com/Disappeared_Race,_New_Fairy
 * @author Dmitri
 */
public class Q00823_DisappearedRaceNewFairy extends Quest
{
	// NPCs
	private static final int MIMYU = 30747;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23566, // Nymph Rose
		23567, // Nymph Rose
		23568, // Nymph Lily
		23569, // Nymph Lily
		23570, // Nymph Tulip
		23571, // Nymph Tulip
		23572, // Nymph Cosmos
		23573, // Nymph Cosmos
		23578 // Nymph Guardian
	};
	
	// Items
	private static final int NYMPH_STAMEN = 46258;
	private static final int BASIC_SUPPLY_BOX = 47178;
	private static final int INTERMEDIATE_SUPPLY_BOX = 47179;
	private static final int ADVANCED_SUPPLY_BOX = 47180;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	public Q00823_DisappearedRaceNewFairy()
	{
		super(823);
		addStartNpc(MIMYU);
		addTalkId(MIMYU);
		addKillId(MONSTERS);
		registerQuestItems(NYMPH_STAMEN);
		addCondMinLevel(MIN_LEVEL, "30747-00.htm");
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
			case "30747-02.htm":
			case "30747-03.htm":
			case "30747-04.htm":
			case "30747-04a.htm":
			case "30747-04b.htm":
			case "30747-06.html":
			case "30747-06a.html":
			case "30747-06b.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 1) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 2))
				{
					htmltext = "30747-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 2)
				{
					htmltext = "30747-04b.htm";
					break;
				}
				
				htmltext = "30747-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 1) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 2))
				{
					htmltext = "30747-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 2)
				{
					htmltext = "30747-04b.htm";
					break;
				}
				
				htmltext = "30747-04.htm";
				break;
			}
			case "30747-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "30747-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "30747-07b.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "30747-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 5:
					{
						if ((getQuestItemsCount(player, NYMPH_STAMEN) == 100) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 5_536_944_000L, 13_288_590);
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
						if ((getQuestItemsCount(player, NYMPH_STAMEN) == 200) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 11_073_888_000L, 26_577_180);
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
						if ((getQuestItemsCount(player, NYMPH_STAMEN) == 300) && (player.getLevel() >= MIN_LEVEL))
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
							
							addExpAndSp(player, 16_610_832_000L, 39_865_770);
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
				htmltext = "30747-01.htm";
				// fallthrough?
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 1) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 2))
						{
							htmltext = "30747-04a.htm";
							break;
						}
						else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 2)
						{
							htmltext = "30747-04b.htm";
							break;
						}
						
						htmltext = "30747-04.htm";
						break;
					}
					case 2:
					{
						htmltext = "30747-08.html";
						break;
					}
					case 3:
					{
						htmltext = "30747-08a.html";
						break;
					}
					case 4:
					{
						htmltext = "30747-08b.html";
						break;
					}
					case 5:
					case 6:
					case 7:
					{
						htmltext = "30747-09.html";
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
					htmltext = "30747-01.htm";
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
					if (giveItemRandomly(player, npc, NYMPH_STAMEN, 1, 100, 1, true))
					{
						qs.setCond(5, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, NYMPH_STAMEN, 1, 200, 1, true))
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 4:
				{
					if (giveItemRandomly(player, npc, NYMPH_STAMEN, 1, 300, 1, true))
					{
						qs.setCond(7, true);
					}
					break;
				}
			}
		}
	}
}
