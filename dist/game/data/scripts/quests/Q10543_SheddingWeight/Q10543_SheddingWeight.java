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
package quests.Q10543_SheddingWeight;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.Q10542_SearchingForNewPower.Q10542_SearchingForNewPower;

/**
 * Shedding Weight (10543)
 * @URL https://l2wiki.com/Shedding_Weight
 * @author Gigi
 */
public class Q10543_SheddingWeight extends Quest
{
	// NPCs
	private static final int SHANNON = 32974;
	private static final int WILFORD = 30005;
	
	// Items
	private static final int NOVICE_TRAINING_LOG = 47601;
	private static final int APPRENTICE_ADVENTURERS_STAFF = 7816;
	private static final int APPRENTICE_ADVENTURERS_BONE_CLUB = 7817;
	private static final int APPRENTICE_ADVENTURERS_KNIFE = 7818;
	private static final int APPRENTICE_ADVENTURERS_CESTUS = 7819;
	private static final int APPRENTICE_ADVENTURERS_BOW = 7820;
	private static final int APPRENTICE_ADVENTURERS_LONG_SWORD = 7821;
	
	// Misc
	private static final int MAX_LEVEL = 20;
	
	public Q10543_SheddingWeight()
	{
		super(10543);
		addStartNpc(SHANNON);
		addTalkId(SHANNON, WILFORD);
		registerQuestItems(NOVICE_TRAINING_LOG);
		addCondNotRace(Race.ERTHEIA, "noRace.html");
		addCondMaxLevel(MAX_LEVEL, "noLevel.html");
		addCondCompletedQuest(Q10542_SearchingForNewPower.class.getSimpleName(), "noLevel.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = null;
		switch (event)
		{
			case "32974-02.htm":
			case "30005-02.html":
			{
				htmltext = event;
				break;
			}
			case "32974-03.htm":
			{
				qs.startQuest();
				qs.setCond(2); // arrow hack
				qs.setCond(1);
				giveItems(player, NOVICE_TRAINING_LOG, 1);
				htmltext = event;
				break;
			}
			case "30005-03.html":
			{
				giveItems(player, APPRENTICE_ADVENTURERS_STAFF, 1);
				giveItems(player, APPRENTICE_ADVENTURERS_BONE_CLUB, 1);
				giveItems(player, APPRENTICE_ADVENTURERS_KNIFE, 1);
				giveItems(player, APPRENTICE_ADVENTURERS_CESTUS, 1);
				giveItems(player, APPRENTICE_ADVENTURERS_BOW, 1);
				giveItems(player, APPRENTICE_ADVENTURERS_LONG_SWORD, 1);
				player.sendPacket(new ExTutorialShowId(24));
				showOnScreenMsg(player, NpcStringId.WEAPONS_HAVE_BEEN_ADDED_TO_YOUR_INVENTORY, ExShowScreenMessage.TOP_CENTER, 10000);
				addExpAndSp(player, 3156, 9);
				qs.exitQuest(false, true);
				htmltext = event;
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
				if (npc.getId() == SHANNON)
				{
					htmltext = "32974-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case SHANNON:
					{
						if (qs.isCond(1))
						{
							htmltext = "32974-04.html";
						}
						else if (qs.isCond(5))
						{
							htmltext = "32974-05.html";
						}
						break;
					}
					case WILFORD:
					{
						if (qs.isCond(1))
						{
							htmltext = "30005-01.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
}
