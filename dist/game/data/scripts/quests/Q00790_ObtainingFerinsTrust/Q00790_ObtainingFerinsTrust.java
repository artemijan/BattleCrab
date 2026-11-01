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
package quests.Q00790_ObtainingFerinsTrust;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Obtaining Ferin's Trust (790)
 * @URL https://l2wiki.com/Obtaining_Ferin%27s_Trust
 * @author Gigi
 */
public class Q00790_ObtainingFerinsTrust extends Quest
{
	// NPCs
	private static final int CYPHONA = 34055;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23541, // Kerberos Lager
		23550, // Kerberos Lager (night)
		23542, // Kerberos Fort
		23551, // Kerberos Fort (night)
		23543, // Kerberos Nero
		23552, // Kerberos Nero (night)
		23544, // Fury Sylph Barrena
		23553, // Fury Sylph Barrena (night)
		23546, // Fury Sylph Temptress
		23555, // Fury Sylph Temptress (night)
		23547, // Fury Sylph Purka
		23556, // Fury Sylph Purka (night)
		23545, // Fury Kerberos Leger
		23557, // Fury Kerberos Leger (night)
		23549, // Fury Kerberos Nero
		23558 // Fury Kerberos Nero (night)
	};
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	// Item's
	private static final int MUTATED_SPIRITS_SOUL = 45849;
	private static final int UNWORLDLY_VISITORS_BASIC_SUPPLY_BOX = 47181;
	private static final int UNWORLDLY_VISITORS_INTERMEDIATE_SUPPLY_BOX = 47182;
	private static final int UNWORLDLY_VISITORS_ADVANCED_SUPPLY_BOX = 47183;
	
	public Q00790_ObtainingFerinsTrust()
	{
		super(790);
		addStartNpc(CYPHONA);
		addTalkId(CYPHONA);
		addKillId(MONSTERS);
		registerQuestItems(MUTATED_SPIRITS_SOUL);
		addCondMinLevel(MIN_LEVEL, "34055-00.htm");
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
			case "34055-02.htm":
			case "34055-03.htm":
			case "34055-04.htm":
			case "34055-04a.htm":
			case "34055-04b.htm":
			case "34055-06.html":
			case "34055-06a.html":
			case "34055-06b.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.UNWORLDLY_VISITORS) >= 1) && (player.getFactionLevel(Faction.UNWORLDLY_VISITORS) < 2))
				{
					htmltext = "34055-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.UNWORLDLY_VISITORS) >= 2)
				{
					htmltext = "34055-04b.htm";
					break;
				}
				
				htmltext = "34055-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.UNWORLDLY_VISITORS) >= 1) && (player.getFactionLevel(Faction.UNWORLDLY_VISITORS) < 2))
				{
					htmltext = "34055-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.UNWORLDLY_VISITORS) >= 2)
				{
					htmltext = "34055-04b.htm";
					break;
				}
				
				htmltext = "34055-04.htm";
				break;
			}
			case "34055-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34055-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34055-07b.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34055-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 5:
					{
						if ((getQuestItemsCount(player, MUTATED_SPIRITS_SOUL) == 200) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 2)
							{
								giveItems(player, UNWORLDLY_VISITORS_ADVANCED_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, UNWORLDLY_VISITORS_INTERMEDIATE_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, UNWORLDLY_VISITORS_BASIC_SUPPLY_BOX, 1);
							}
							
							addExpAndSp(player, 22_221_427_950L, 22_221_360);
							addFactionPoints(player, Faction.UNWORLDLY_VISITORS, 100);
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
						if ((getQuestItemsCount(player, MUTATED_SPIRITS_SOUL) == 400) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 2)
							{
								giveItems(player, UNWORLDLY_VISITORS_ADVANCED_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, UNWORLDLY_VISITORS_BASIC_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, UNWORLDLY_VISITORS_INTERMEDIATE_SUPPLY_BOX, 1);
							}
							
							addExpAndSp(player, 44_442_855_900L, 44_442_720);
							addFactionPoints(player, Faction.UNWORLDLY_VISITORS, 200);
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
						if ((getQuestItemsCount(player, MUTATED_SPIRITS_SOUL) == 600) && (player.getLevel() >= MIN_LEVEL))
						{
							if (chance < 2)
							{
								giveItems(player, UNWORLDLY_VISITORS_BASIC_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, UNWORLDLY_VISITORS_INTERMEDIATE_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, UNWORLDLY_VISITORS_ADVANCED_SUPPLY_BOX, 1);
							}
							
							addExpAndSp(player, 66_664_283_850L, 66_664_080);
							addFactionPoints(player, Faction.UNWORLDLY_VISITORS, 300);
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
				htmltext = "34055-01.htm";
				// fallthrough?
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.UNWORLDLY_VISITORS) >= 1) && (player.getFactionLevel(Faction.UNWORLDLY_VISITORS) < 2))
						{
							htmltext = "34055-04a.htm";
							break;
						}
						else if (player.getFactionLevel(Faction.UNWORLDLY_VISITORS) >= 2)
						{
							htmltext = "34055-04b.htm";
							break;
						}
						
						htmltext = "34055-04.htm";
						break;
					}
					case 2:
					{
						htmltext = "34055-08.html";
						break;
					}
					case 3:
					{
						htmltext = "34055-08a.html";
						break;
					}
					case 4:
					{
						htmltext = "34055-08b.html";
						break;
					}
					case 5:
					case 6:
					case 7:
					{
						htmltext = "34055-09.html";
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
					htmltext = "34055-01.htm";
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
					if (giveItemRandomly(player, npc, MUTATED_SPIRITS_SOUL, 1, 200, 1, true))
					{
						qs.setCond(5, true);
					}
					break;
				}
				case 3:
				{
					if (giveItemRandomly(player, npc, MUTATED_SPIRITS_SOUL, 1, 400, 1, true))
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 4:
				{
					if (giveItemRandomly(player, npc, MUTATED_SPIRITS_SOUL, 1, 600, 1, true))
					{
						qs.setCond(7, true);
					}
					break;
				}
			}
		}
	}
}
