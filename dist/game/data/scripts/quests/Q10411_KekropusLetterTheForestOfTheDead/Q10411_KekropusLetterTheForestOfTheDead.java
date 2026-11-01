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
package quests.Q10411_KekropusLetterTheForestOfTheDead;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.LetterQuest;

/**
 * Kekropus' Letter: The Forest of the Dead (10411)
 * @author St3eT
 */
public class Q10411_KekropusLetterTheForestOfTheDead extends LetterQuest
{
	// NPCs
	private static final int MATHIAS = 31340;
	private static final int HATUBA = 33849;
	private static final int INVISIBLE_NPC = 19543;
	
	// Items
	private static final int SOE_FOREST_OF_THE_DEAD = 37031; // Scroll of Escape: Forest of the Dead
	private static final int SOE_TOWN_OF_RUNE = 37118; // Scroll of Escape: Town of Rune
	
	// Location
	private static final Location TELEPORT_LOC = new Location(42682, -47986, -792);
	
	// Misc
	private static final int MIN_LEVEL = 65;
	private static final int MAX_LEVEL = 69;
	
	public Q10411_KekropusLetterTheForestOfTheDead()
	{
		super(10411);
		addTalkId(MATHIAS, HATUBA);
		addCreatureSeeId(INVISIBLE_NPC);
		setIsErtheiaQuest(false);
		setLevel(MIN_LEVEL, MAX_LEVEL);
		setStartQuestSound("Npcdialog1.kekrops_quest_7");
		setStartLocation(SOE_TOWN_OF_RUNE, TELEPORT_LOC);
		registerQuestItems(SOE_TOWN_OF_RUNE, SOE_FOREST_OF_THE_DEAD);
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
			case "31340-02.html":
			{
				htmltext = event;
				break;
			}
			case "31340-03.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
					giveItems(player, SOE_FOREST_OF_THE_DEAD, 1);
					htmltext = event;
				}
				break;
			}
			case "33849-02.html":
			{
				if (qs.isCond(3))
				{
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 942690, 226);
					}
					
					showOnScreenMsg(player, NpcStringId.GROW_STRONGER_HERE_UNTIL_YOU_RECEIVE_THE_NEXT_LETTER_FROM_KEKROPUS_AT_LV_70, ExShowScreenMessage.TOP_CENTER, 6000);
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
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		if (qs.isStarted())
		{
			if ((npc.getId() == MATHIAS) && qs.isCond(2))
			{
				htmltext = "31340-01.html";
			}
			else if (qs.isCond(3))
			{
				htmltext = npc.getId() == MATHIAS ? "31340-04.html" : "33849-01.html";
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
				showOnScreenMsg(player, NpcStringId.FOREST_OF_THE_DEAD_IS_A_GOOD_HUNTING_ZONE_FOR_LV_65_OR_ABOVE, ExShowScreenMessage.TOP_CENTER, 6000);
			}
		}
	}
	
	@Override
	public boolean canShowTutorialMark(Player player)
	{
		return player.isMageClass();
	}
}
