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
package quests.Q00787_TheRoleOfAWatcher;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * The Role of a Watcher (00787)
 * @URL https://l2wiki.com/The_Role_of_a_Watcher
 * @author Stayway
 */
public class Q00787_TheRoleOfAWatcher extends Quest
{
	// NPCs
	private static final int NAMO = 33973;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23423, // Mesmer Dragon
		23424, // Gargoyle Dragon
		23425, // Black Dragon
		23427, // Sand Dragon
		23428, // Captain Dragonblood
		23429, // Minion Dragonblood
		23436, // Cave Servant Archer
		23437, // Cave Servant Warrior
		23438, // Metallic Cave Servant
		23439, // Iron Cave Servant
		23440, // Headless Knight
		23430, // Prey Drake
		23431, // Beast Drake
		23432, // Dust Drake
		23433, // Vampiric Drake
		23441, // Bloody Grave Warrior
		23442, // Dark Grave Warrior
		23443, // Dark Grave Wizard
		23444, // Dark Grave Knight
	};
	
	// Items
	private static final int DRAGON_BONE_DUST = 39736; // min 50
	private static final int DRAGON_BONE_FRAGMENT = 39737; // max 900
	private static final int EMISSARY_REWARD_BOX = 39728; // Emissary's Reward Box (High-grade)
	
	// Misc
	private static final int MIN_LEVEL = 81;
	private static final int MAX_LEVEL = 84;
	
	public Q00787_TheRoleOfAWatcher()
	{
		super(787);
		addStartNpc(NAMO);
		addTalkId(NAMO);
		addKillId(MONSTERS);
		registerQuestItems(DRAGON_BONE_DUST, DRAGON_BONE_FRAGMENT);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "no_level.html");
		addCondRace(Race.ERTHEIA, "noErtheia.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "33973-02.htm":
			case "33973-03.htm":
			case "33973-07.html":
			case "33973-08.html":
			{
				htmltext = event;
				break;
			}
			case "33973-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33973-09.html":
			{
				if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) < 100))
				{
					addExpAndSp(player, 14140350, 3393);
					giveItems(player, EMISSARY_REWARD_BOX, 1);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && ((getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 100) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) <= 199)))
				{
					addExpAndSp(player, 28280700, 6786);
					giveItems(player, EMISSARY_REWARD_BOX, 2);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && ((getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 200) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) <= 299)))
				{
					addExpAndSp(player, 42421050, 10179);
					giveItems(player, EMISSARY_REWARD_BOX, 3);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && ((getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 300) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) <= 399)))
				{
					addExpAndSp(player, 56561400, 13572);
					giveItems(player, EMISSARY_REWARD_BOX, 4);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && ((getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 400) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) <= 499)))
				{
					addExpAndSp(player, 70701750, 16965);
					giveItems(player, EMISSARY_REWARD_BOX, 5);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && ((getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 500) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) <= 599)))
				{
					addExpAndSp(player, 84842100, 20358);
					giveItems(player, EMISSARY_REWARD_BOX, 6);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && ((getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 600) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) <= 699)))
				{
					addExpAndSp(player, 98982450, 23751);
					giveItems(player, EMISSARY_REWARD_BOX, 7);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && ((getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 700) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) <= 799)))
				{
					addExpAndSp(player, 113122800, 27144);
					giveItems(player, EMISSARY_REWARD_BOX, 8);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && ((getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 800) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) <= 899)))
				{
					addExpAndSp(player, 127263150, 30537);
					giveItems(player, EMISSARY_REWARD_BOX, 9);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				
				if ((getQuestItemsCount(player, DRAGON_BONE_DUST) >= 50) && (getQuestItemsCount(player, DRAGON_BONE_FRAGMENT) >= 900))
				{
					addExpAndSp(player, 141403500, 33930);
					giveItems(player, EMISSARY_REWARD_BOX, 10);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		if (npc.getId() == NAMO)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "33973-10.html";
						break;
					}
					
					qs.setState(State.CREATED);
					// fallthrough
				}
				case State.CREATED:
				{
					htmltext = "33973-01.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "33973-05.html";
					}
					else if (qs.isCond(2))
					{
						htmltext = "33973-06.html";
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && (qs.isCond(1)) && giveItemRandomly(killer, npc, DRAGON_BONE_DUST, 1, 50, 0.15, true))
		{
			qs.setCond(2, true);
		}
		
		if ((qs != null) && (qs.isCond(2)) && giveItemRandomly(killer, npc, DRAGON_BONE_FRAGMENT, 1, 900, 0.25, true))
		{
			qs.setCond(2, true);
		}
	}
}
