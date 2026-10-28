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
package quests.Q00032_AnObviousLie;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;

/**
 * An Obvious Lie (32).
 * @author Janiko
 */
public class Q00032_AnObviousLie extends Quest
{
	// NPCs
	private static final int MAXIMILIAN = 30120;
	private static final int GENTLER = 30094;
	private static final int MIKI_THE_CAT = 31706;
	
	// Monster
	private static final int ALLIGATOR = 20135;
	
	// Items
	private static final int MAP_OF_GENTLER = 7165;
	private static final ItemHolder MEDICINAL_HERB = new ItemHolder(7166, 20);
	private static final ItemHolder SPIRIT_ORE = new ItemHolder(3031, 500);
	private static final ItemHolder ARMOR_FRAGMENTS = new ItemHolder(36551, 70);
	private static final ItemHolder GEMS = new ItemHolder(36556, 145);
	
	// Misc
	private static final int MIN_LEVEL = 45;
	
	// Reward
	private static final Map<String, Integer> EARS = new HashMap<>();
	static
	{
		EARS.put("cat", 6843); // Cat Ears
		EARS.put("raccoon", 7680); // Raccoon ears
		EARS.put("rabbit", 7683); // Rabbit ears
	}
	
	public Q00032_AnObviousLie()
	{
		super(32);
		addStartNpc(MAXIMILIAN);
		addTalkId(MAXIMILIAN, GENTLER, MIKI_THE_CAT);
		addKillId(ALLIGATOR);
		registerQuestItems(MAP_OF_GENTLER, MEDICINAL_HERB.getId());
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
			case "30120-02.html":
			{
				if (qs.isCreated())
				{
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "30094-02.html":
			{
				if (qs.isCond(1))
				{
					giveItems(player, MAP_OF_GENTLER, 1);
					qs.setCond(2, true);
					htmltext = event;
				}
				break;
			}
			case "31706-02.html":
			{
				if (qs.isCond(2) && hasQuestItems(player, MAP_OF_GENTLER))
				{
					takeItems(player, MAP_OF_GENTLER, -1);
					qs.setCond(3, true);
					htmltext = event;
				}
				break;
			}
			case "30094-06.html":
			{
				if (qs.isCond(4) && hasItem(player, MEDICINAL_HERB))
				{
					takeItem(player, MEDICINAL_HERB);
					qs.setCond(5, true);
					htmltext = event;
				}
				break;
			}
			case "30094-09.html":
			{
				if (qs.isCond(5) && hasItem(player, SPIRIT_ORE))
				{
					takeItem(player, SPIRIT_ORE);
					qs.setCond(6, true);
					htmltext = event;
				}
				break;
			}
			case "30094-12.html":
			{
				if (qs.isCond(7))
				{
					qs.setCond(8, true);
					htmltext = event;
				}
				break;
			}
			case "30094-15.html":
			{
				htmltext = event;
				break;
			}
			case "31706-05.html":
			{
				if (qs.isCond(6))
				{
					qs.setCond(7, true);
					htmltext = event;
				}
				break;
			}
			case "cat":
			case "raccoon":
			case "rabbit":
			{
				if (qs.isCond(8) && takeAllItems(player, ARMOR_FRAGMENTS, GEMS))
				{
					giveItems(player, EARS.get(event), 1);
					qs.exitQuest(false, true);
					htmltext = "30094-16.html";
				}
				else
				{
					htmltext = "30094-17.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 3, 3, npc);
		if ((qs != null) && giveItemRandomly(qs.getPlayer(), npc, MEDICINAL_HERB.getId(), 1, MEDICINAL_HERB.getCount(), 1, true))
		{
			qs.setCond(4);
		}
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (npc.getId())
		{
			case MAXIMILIAN:
			{
				if (qs.isCreated())
				{
					htmltext = (player.getLevel() >= MIN_LEVEL) ? "30120-01.htm" : "30120-03.htm";
				}
				else if (qs.isStarted())
				{
					if (qs.isCond(1))
					{
						htmltext = "30120-04.html";
					}
				}
				else
				{
					htmltext = getAlreadyCompletedMsg(player);
				}
				break;
			}
			case GENTLER:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "30094-01.html";
						break;
					}
					case 2:
					{
						htmltext = "30094-03.html";
						break;
					}
					case 4:
					{
						htmltext = hasItem(player, MEDICINAL_HERB) ? "30094-04.html" : "30094-05.html";
						break;
					}
					case 5:
					{
						htmltext = hasItem(player, SPIRIT_ORE) ? "30094-07.html" : "30094-08.html";
						break;
					}
					case 6:
					{
						htmltext = "30094-10.html";
						break;
					}
					case 7:
					{
						htmltext = "30094-11.html";
						break;
					}
					case 8:
					{
						if (hasAllItems(player, true, ARMOR_FRAGMENTS, GEMS))
						{
							htmltext = "30094-13.html";
						}
						else
						{
							htmltext = "30094-14.html";
						}
						break;
					}
				}
				break;
			}
			case MIKI_THE_CAT:
			{
				switch (qs.getCond())
				{
					case 2:
					{
						if (hasQuestItems(player, MAP_OF_GENTLER))
						{
							htmltext = "31706-01.html";
						}
						break;
					}
					case 3:
					case 4:
					case 5:
					{
						htmltext = "31706-03.html";
						break;
					}
					case 6:
					{
						htmltext = "31706-04.html";
						break;
					}
					case 7:
					{
						htmltext = "31706-06.html";
						break;
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
}
