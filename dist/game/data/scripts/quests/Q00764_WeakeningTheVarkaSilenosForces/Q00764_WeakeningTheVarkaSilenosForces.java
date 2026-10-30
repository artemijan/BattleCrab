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
package quests.Q00764_WeakeningTheVarkaSilenosForces;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Weakening the Varka Silenos Forces (764)
 * @URL https://l2wiki.com/Weakening_the_Varka_Silenos_Forces
 * @author Gigi
 */
public class Q00764_WeakeningTheVarkaSilenosForces extends Quest
{
	// NPC
	private static final int HANSEN = 33853;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		21350, // Varka Silenos Recruit
		21353, // Varka Silenos Scout
		21354, // Varka Silenos Hunter
		21355, // Varka Silenos Shaman
		21357, // Varka Silenos Priest
		21358, // Varka Silenos Warrior
		21360, // Varka Silenos Medium
		21362, // Varka Silenos Officer
		21364, // Varka Silenos Seer
		21365, // Varka Silenos Great Magus
		21366, // Varka Silenos General
		21368, // Varka Silenos Great Seer
		21369, // Varka's Commander
		21371, // Varka's Head Magus
		21373 // Varka's Prophet
	};
	
	// Items
	private static final int BADGE_SOLDIER = 36674;
	private static final int BADGE_GENERAL = 36675;
	
	// Rewards
	private static final int STEEL_DOOR_GUILD_BOX = 37393;
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 80;
	
	public Q00764_WeakeningTheVarkaSilenosForces()
	{
		super(764);
		addStartNpc(HANSEN);
		addTalkId(HANSEN);
		addKillId(MONSTERS);
		registerQuestItems(BADGE_SOLDIER, BADGE_GENERAL);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33853-00.htm");
		addCondNotRace(Race.ERTHEIA, "33853-07.html");
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
			case "33853-02.htm":
			case "33853-03.htm":
			case "33853-09.html":
			case "33853-09a.html":
			case "33853-10.html":
			{
				htmltext = event;
				break;
			}
			case "33853-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33853-06a.html":
			{
				if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && (getQuestItemsCount(player, BADGE_GENERAL) < 100))
				{
					addExpAndSp(player, 19164600, 191646);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 1);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && ((getQuestItemsCount(player, BADGE_GENERAL) >= 100) && (getQuestItemsCount(player, BADGE_GENERAL) <= 199)))
				{
					addExpAndSp(player, 38329200, 383292);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 2);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && ((getQuestItemsCount(player, BADGE_GENERAL) >= 200) && (getQuestItemsCount(player, BADGE_GENERAL) <= 299)))
				{
					addExpAndSp(player, 57493800, 574938);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 3);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && ((getQuestItemsCount(player, BADGE_GENERAL) >= 300) && (getQuestItemsCount(player, BADGE_GENERAL) <= 399)))
				{
					addExpAndSp(player, 76658400, 766584);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 4);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && ((getQuestItemsCount(player, BADGE_GENERAL) >= 400) && (getQuestItemsCount(player, BADGE_GENERAL) <= 499)))
				{
					addExpAndSp(player, 95823000, 958230);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 5);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && ((getQuestItemsCount(player, BADGE_GENERAL) >= 500) && (getQuestItemsCount(player, BADGE_GENERAL) <= 599)))
				{
					addExpAndSp(player, 114987600, 1149876);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 6);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && ((getQuestItemsCount(player, BADGE_GENERAL) >= 600) && (getQuestItemsCount(player, BADGE_GENERAL) <= 699)))
				{
					addExpAndSp(player, 134152200, 1341522);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 7);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && ((getQuestItemsCount(player, BADGE_GENERAL) >= 700) && (getQuestItemsCount(player, BADGE_GENERAL) <= 799)))
				{
					addExpAndSp(player, 153316800, 1533168);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 8);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && ((getQuestItemsCount(player, BADGE_GENERAL) >= 800) && (getQuestItemsCount(player, BADGE_GENERAL) <= 899)))
				{
					addExpAndSp(player, 172481400, 1724814);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 9);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, BADGE_SOLDIER) >= 50) && (getQuestItemsCount(player, BADGE_GENERAL) >= 900))
				{
					addExpAndSp(player, 191646000, 1916460);
					giveItems(player, STEEL_DOOR_GUILD_BOX, 10);
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
		if ((npc.getId() == HANSEN) && player.isMageClass())
		{
			return "33853-00.htm";
		}
		
		if (npc.getId() == HANSEN)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "33853-06.html";
						break;
					}
					
					qs.setState(State.CREATED);
					break;
				}
				case State.CREATED:
				{
					htmltext = "33853-01.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "33853-05.html";
					}
					else if (qs.isStarted() && qs.isCond(2))
					{
						htmltext = "33853-08.html";
					}
					break;
				}
			}
		}
		else if (qs.isCompleted() && !qs.isNowAvailable())
		{
			htmltext = "33853-06.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && (giveItemRandomly(killer, npc, BADGE_SOLDIER, 1, 50, 0.15, true)))
		{
			qs.setCond(2, true);
		}
		
		if ((qs != null) && qs.isCond(2) && (giveItemRandomly(killer, npc, BADGE_GENERAL, 1, 900, 0.85, true)))
		{
			qs.setCond(2, true);
		}
	}
}
