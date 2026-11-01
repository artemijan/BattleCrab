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
package quests.Q00344_1000YearsTheEndOfLamentation;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;

/**
 * 1000 years, the End of Lamentation (344)
 * @author Mobius
 */
public class Q00344_1000YearsTheEndOfLamentation extends Quest
{
	// NPCs
	private static final int KAIEN = 30623;
	private static final int GARVARENTZ = 30704;
	private static final int GILMORE = 30754;
	private static final int RODEMAI = 30756;
	private static final int ORVEN = 30857;
	
	// Items
	private static final int ARTICLES = 4269;
	private static final ItemHolder OLD_KEY = new ItemHolder(4270, 1);
	private static final ItemHolder OLD_HILT = new ItemHolder(4271, 1);
	private static final ItemHolder TOTEM_NECKLACE = new ItemHolder(4272, 1);
	private static final ItemHolder CRUCIFIX = new ItemHolder(4273, 1);
	
	// Monsters
	private static final Map<Integer, Double> MONSTER_CHANCES = new HashMap<>();
	static
	{
		MONSTER_CHANCES.put(20236, 0.58); // Cave Servant
		MONSTER_CHANCES.put(20238, 0.75); // Cave Servant Warrior
		MONSTER_CHANCES.put(20237, 0.78); // Cave Servant Archer
		MONSTER_CHANCES.put(20239, 0.79); // Cave Servant Captain
		MONSTER_CHANCES.put(20240, 0.85); // Royal Cave Servant
		MONSTER_CHANCES.put(20272, 0.58); // Cave Servant
		MONSTER_CHANCES.put(20273, 0.78); // Cave Servant Archer
		MONSTER_CHANCES.put(20274, 0.75); // Cave Servant Warrior
		MONSTER_CHANCES.put(20275, 0.79); // Cave Servant Captain
		MONSTER_CHANCES.put(20276, 0.85); // Royal Cave Servant
	}
	
	// Rewards
	private static final ItemHolder ORIHARUKON_ORE = new ItemHolder(1874, 25);
	private static final ItemHolder VARNISH_OF_PURITY = new ItemHolder(1887, 10);
	private static final ItemHolder SCROLL_EWC = new ItemHolder(951, 1);
	private static final ItemHolder RAID_SWORD = new ItemHolder(133, 1);
	private static final ItemHolder COKES = new ItemHolder(1879, 55);
	private static final ItemHolder RING_OF_AGES = new ItemHolder(885, 1);
	private static final ItemHolder LEATHER = new ItemHolder(1882, 70);
	private static final ItemHolder COARSE_BONE_POWDER = new ItemHolder(1881, 50);
	private static final ItemHolder HEAVY_DOOM_HAMMER = new ItemHolder(191, 1);
	private static final ItemHolder STONE_OF_PURITY = new ItemHolder(1875, 19);
	private static final ItemHolder SCROLL_EAC = new ItemHolder(952, 5);
	private static final ItemHolder DRAKE_LEATHER_BOOTS = new ItemHolder(2437, 1);
	
	// Misc
	private static final int MIN_LEVEL = 48;
	private static final int MAX_LEVEL = 55;
	
	public Q00344_1000YearsTheEndOfLamentation()
	{
		super(344);
		addStartNpc(GILMORE);
		addTalkId(KAIEN, GARVARENTZ, GILMORE, RODEMAI, ORVEN);
		addKillId(MONSTER_CHANCES.keySet());
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "nolevel.html");
		registerQuestItems(ARTICLES, OLD_KEY.getId(), OLD_HILT.getId(), TOTEM_NECKLACE.getId(), CRUCIFIX.getId());
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		String htmltext = null;
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "30754-03.htm":
			case "30754-16.html":
			{
				htmltext = event;
				break;
			}
			case "30754-04.htm":
			{
				if (qs.isCreated())
				{
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "30754-08.html":
			{
				if (qs.isCond(1))
				{
					final long count = getQuestItemsCount(player, ARTICLES);
					if (count < 1)
					{
						htmltext = "30754-07.html";
					}
					else
					{
						takeItems(player, ARTICLES, -1);
						if (getRandom(1000) >= count)
						{
							giveAdena(player, count * 60, true);
							htmltext = event;
						}
						else
						{
							qs.setCond(2, true);
							switch (getRandom(4))
							{
								case 0:
								{
									qs.setMemoState(1);
									giveItems(player, OLD_HILT);
									break;
								}
								case 1:
								{
									qs.setMemoState(2);
									giveItems(player, OLD_KEY);
									break;
								}
								case 2:
								{
									qs.setMemoState(3);
									giveItems(player, TOTEM_NECKLACE);
									break;
								}
								case 3:
								{
									qs.setMemoState(4);
									giveItems(player, CRUCIFIX);
									break;
								}
							}
							
							htmltext = "30754-09.html";
						}
					}
				}
				break;
			}
			case "30754-17.html":
			{
				if (qs.isCond(1))
				{
					htmltext = event;
					qs.exitQuest(true, true);
				}
				break;
			}
			case "relic_info":
			{
				switch (qs.getMemoState())
				{
					case 1:
					{
						htmltext = "30754-10.html";
						break;
					}
					case 2:
					{
						htmltext = "30754-11.html";
						break;
					}
					case 3:
					{
						htmltext = "30754-12.html";
						break;
					}
					case 4:
					{
						htmltext = "30754-13.html";
						break;
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		final QuestState qs = getQuestState(talker, true);
		String htmltext = getNoQuestMsg(talker);
		switch (npc.getId())
		{
			case GILMORE:
			{
				if (qs.isCreated())
				{
					htmltext = ((talker.getLevel() >= MIN_LEVEL) && (talker.getLevel() <= MAX_LEVEL)) ? "30754-02.htm" : "30754-01.htm";
				}
				else if (qs.isStarted())
				{
					if (qs.isCond(1))
					{
						htmltext = (hasQuestItems(talker, ARTICLES)) ? "30754-06.html" : "30754-05.html";
					}
					else if (hasItem(talker, OLD_KEY) || hasItem(talker, OLD_HILT) || hasItem(talker, TOTEM_NECKLACE) || hasItem(talker, CRUCIFIX))
					{
						htmltext = "30754-14.html";
					}
					else
					{
						qs.setCond(1);
						htmltext = "30754-15.html";
					}
				}
				else
				{
					htmltext = getAlreadyCompletedMsg(talker);
				}
				break;
			}
			case KAIEN:
			{
				if (qs.getMemoState() == 1)
				{
					if (hasItem(talker, OLD_HILT))
					{
						takeItems(talker, OLD_HILT.getId(), -1);
						final int rand = getRandom(100);
						if (rand <= 52)
						{
							rewardItems(talker, ORIHARUKON_ORE);
						}
						else if (rand <= 76)
						{
							rewardItems(talker, VARNISH_OF_PURITY);
						}
						else if (rand <= 98)
						{
							rewardItems(talker, SCROLL_EWC);
						}
						else
						{
							rewardItems(talker, RAID_SWORD);
						}
						
						qs.setCond(1);
						htmltext = "30623-01.html";
					}
					else
					{
						htmltext = "30623-02.html";
					}
				}
				break;
			}
			case RODEMAI:
			{
				if (qs.getMemoState() == 2)
				{
					if (hasItem(talker, OLD_KEY))
					{
						takeItems(talker, OLD_KEY.getId(), -1);
						final int rand = getRandom(100);
						if (rand <= 39)
						{
							rewardItems(talker, COKES);
						}
						else if (rand <= 89)
						{
							rewardItems(talker, SCROLL_EWC);
						}
						else
						{
							rewardItems(talker, RING_OF_AGES);
						}
						
						qs.setCond(1);
						htmltext = "30756-01.html";
					}
					else
					{
						htmltext = "30756-02.html";
					}
				}
				break;
			}
			case GARVARENTZ:
			{
				if (qs.getMemoState() == 3)
				{
					if (hasItem(talker, TOTEM_NECKLACE))
					{
						takeItems(talker, TOTEM_NECKLACE.getId(), -1);
						final int rand = getRandom(100);
						if (rand <= 47)
						{
							rewardItems(talker, LEATHER);
						}
						else if (rand <= 97)
						{
							rewardItems(talker, COARSE_BONE_POWDER);
						}
						else
						{
							rewardItems(talker, HEAVY_DOOM_HAMMER);
						}
						
						qs.setCond(1);
						htmltext = "30704-01.html";
					}
					else
					{
						htmltext = "30704-02.html";
					}
				}
				break;
			}
			case ORVEN:
			{
				if (qs.getMemoState() == 4)
				{
					if (hasItem(talker, CRUCIFIX))
					{
						takeItems(talker, CRUCIFIX.getId(), -1);
						final int rand = getRandom(100);
						if (rand <= 49)
						{
							rewardItems(talker, STONE_OF_PURITY);
						}
						else if (rand <= 69)
						{
							rewardItems(talker, SCROLL_EAC);
						}
						else
						{
							rewardItems(talker, DRAKE_LEATHER_BOOTS);
						}
						
						qs.setCond(1);
						htmltext = "30857-01.html";
					}
					else
					{
						htmltext = "30857-02.html";
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if (qs != null)
		{
			giveItemRandomly(qs.getPlayer(), npc, ARTICLES, 1, 0, MONSTER_CHANCES.get(npc.getId()), true);
		}
	}
}
