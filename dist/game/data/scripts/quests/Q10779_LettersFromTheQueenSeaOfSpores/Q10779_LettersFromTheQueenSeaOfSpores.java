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
package quests.Q10779_LettersFromTheQueenSeaOfSpores;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.LetterQuest;

/**
 * Letters from the Queen: Sea of Spores (10779)
 * @author malyelfik, Trevor The Third
 */
public class Q10779_LettersFromTheQueenSeaOfSpores extends LetterQuest
{
	// NPCs
	private static final int HOLINT = 30191;
	private static final int ANDY = 33845;
	
	// Items
	private static final int SOE_OREN = 39574;
	private static final int SOE_SEA_OF_SPORES = 39575;
	
	// Location
	private static final Location TELEPORT_LOC = new Location(83633, 53064, -1456);
	
	// Misc
	private static final int MIN_LEVEL = 52;
	private static final int MAX_LEVEL = 57;
	
	public Q10779_LettersFromTheQueenSeaOfSpores()
	{
		super(10779);
		addTalkId(HOLINT, ANDY);
		setIsErtheiaQuest(true);
		setLevel(MIN_LEVEL, MAX_LEVEL);
		setStartLocation(SOE_OREN, TELEPORT_LOC);
		setStartQuestSound("Npcdialog1.serenia_quest_5");
		registerQuestItems(SOE_OREN, SOE_SEA_OF_SPORES);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = event;
		switch (event)
		{
			case "30191-02.html":
			case "33845-02.html":
			{
				break;
			}
			case "30191-03.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
					giveItems(player, SOE_SEA_OF_SPORES, 1);
				}
				break;
			}
			case "33845-03.html":
			{
				if (qs.isCond(3))
				{
					giveStoryQuestReward(npc, player);
					addExpAndSp(player, 635250, 152);
					showOnScreenMsg(player, NpcStringId.GROW_STRONGER_HERE_UNTIL_YOU_RECEIVE_THE_NEXT_LETTER_FROM_QUEEN_NAVARI_AT_LV_58, ExShowScreenMessage.TOP_CENTER, 8000);
					qs.exitQuest(false, true);
				}
				break;
			}
			default:
			{
				htmltext = null;
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
			if (npc.getId() == HOLINT)
			{
				htmltext = (qs.isCond(2)) ? "30191-01.html" : "30191-04.html";
			}
			else if (qs.isCond(3))
			{
				htmltext = "33845-01.html";
			}
		}
		
		return htmltext;
	}
}
