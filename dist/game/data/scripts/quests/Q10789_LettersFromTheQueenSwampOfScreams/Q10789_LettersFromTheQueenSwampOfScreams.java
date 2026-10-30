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
package quests.Q10789_LettersFromTheQueenSwampOfScreams;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.LetterQuest;

/**
 * Letters from the Queen: Swamp of Screams (10789)
 * @author malyelfik
 */
public class Q10789_LettersFromTheQueenSwampOfScreams extends LetterQuest
{
	// NPCs
	private static final int INNOCENTIN = 31328;
	private static final int DOKARA = 33847;
	
	// Items
	private static final int SOE_RUNE = 39580;
	private static final int SOE_SWAMP_OF_SCREAMS = 39581;
	
	// Location
	private static final Location TELEPORT_LOC = new Location(36563, -49178, -1128);
	
	// Misc
	private static final int MIN_LEVEL = 65;
	private static final int MAX_LEVEL = 69;
	
	public Q10789_LettersFromTheQueenSwampOfScreams()
	{
		super(10789);
		addTalkId(INNOCENTIN, DOKARA);
		setIsErtheiaQuest(true);
		setLevel(MIN_LEVEL, MAX_LEVEL);
		setStartQuestSound("Npcdialog1.serenia_quest_9");
		setStartLocation(SOE_RUNE, TELEPORT_LOC);
		registerQuestItems(SOE_RUNE, SOE_SWAMP_OF_SCREAMS);
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
			case "31328-02.html":
			case "33847-02.html":
			{
				htmltext = event;
				break;
			}
			case "31328-03.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
					giveItems(player, SOE_SWAMP_OF_SCREAMS, 1);
					htmltext = event;
				}
				break;
			}
			case "33847-03.html":
			{
				if (qs.isCond(3))
				{
					giveStoryQuestReward(npc, player);
					addExpAndSp(player, 942690, 226);
					showOnScreenMsg(player, NpcStringId.GROW_STRONGER_HERE_UNTIL_YOU_RECEIVE_THE_NEXT_LETTER_FROM_QUEEN_NAVARI_AT_LV_70, ExShowScreenMessage.TOP_CENTER, 8000);
					qs.exitQuest(false, true);
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
		final QuestState qs = getQuestState(player, false);
		String htmltext = getNoQuestMsg(player);
		if (qs == null)
		{
			return htmltext;
		}
		
		if (qs.isStarted())
		{
			if (npc.getId() == INNOCENTIN)
			{
				htmltext = (qs.isCond(2)) ? "31328-01.html" : "31328-04.html";
			}
			else if (qs.isCond(3))
			{
				htmltext = "33847-01.html";
			}
		}
		
		return htmltext;
	}
	
	@Override
	public boolean canShowTutorialMark(Player player)
	{
		return !player.isMageClass();
	}
}
