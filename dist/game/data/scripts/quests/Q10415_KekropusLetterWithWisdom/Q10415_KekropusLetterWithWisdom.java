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
package quests.Q10415_KekropusLetterWithWisdom;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.LetterQuest;

/**
 * Kekropus Letter: With Wisdom (10415)
 * @URL https://l2wiki.com/Kekropus%27_Letter:_With_Wisdom
 * @author Gigi
 */
public class Q10415_KekropusLetterWithWisdom extends LetterQuest
{
	// NPCs
	private static final int ANDREI = 31292;
	private static final int JANITT = 33851;
	private static final int INVISIBLE_NPC = 19543;
	
	// Items
	private static final int SOE_WALL_OF_ARGOS = 37032; // Scroll of Escape: Wall of Argos
	private static final int SOE_TOWN_OF_GODDARD = 37119; // Scroll of Escape: Town of Goddard
	
	// Requirements
	private static final int MIN_LEVEL = 70;
	private static final int MAX_LEVEL = 75;
	
	// Teleport
	private static final Location TELEPORT_LOC = new Location(147491, -56633, -2776);
	
	public Q10415_KekropusLetterWithWisdom()
	{
		super(10415);
		addTalkId(ANDREI, JANITT);
		addCreatureSeeId(INVISIBLE_NPC);
		setIsErtheiaQuest(false);
		setLevel(MIN_LEVEL, MAX_LEVEL);
		setStartQuestSound("Npcdialog1.kekrops_quest_8");
		setStartLocation(SOE_TOWN_OF_GODDARD, TELEPORT_LOC);
		registerQuestItems(SOE_TOWN_OF_GODDARD, SOE_WALL_OF_ARGOS);
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
			case "31292-02.html":
			{
				htmltext = event;
				break;
			}
			case "31292-03.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
					giveItems(player, SOE_WALL_OF_ARGOS, 1);
					htmltext = event;
				}
				break;
			}
			case "33851-02.html":
			{
				if (qs.isCond(3))
				{
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 1088640, 261);
					}
					
					showOnScreenMsg(player, NpcStringId.GROW_STRONGER_HERE_UNTIL_YOU_RECEIVE_THE_NEXT_LETTER_FROM_KEKROPUS_AT_LV_76, ExShowScreenMessage.TOP_CENTER, 6000);
					htmltext = event;
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
		if (qs.isStarted())
		{
			if ((npc.getId() == ANDREI) && qs.isCond(2))
			{
				htmltext = "31292-01.html";
			}
			else if (qs.isCond(3))
			{
				htmltext = npc.getId() == ANDREI ? "31292-04.html" : "33851-01.html";
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer())
		{
			final Player player = creature.asPlayer();
			final QuestState qs = getQuestState(player, false);
			if ((qs != null) && qs.isCond(3))
			{
				showOnScreenMsg(player, NpcStringId.WALL_OF_ARGOS_IS_A_GOOD_HUNTING_ZONE_FOR_LV_70_OR_ABOVE, ExShowScreenMessage.TOP_CENTER, 6000);
			}
		}
	}
	
	@Override
	public boolean canShowTutorialMark(Player player)
	{
		return player.isMageClass();
	}
}
