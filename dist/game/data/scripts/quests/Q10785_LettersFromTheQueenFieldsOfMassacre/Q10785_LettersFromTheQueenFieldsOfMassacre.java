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
package quests.Q10785_LettersFromTheQueenFieldsOfMassacre;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.LetterQuest;

/**
 * Letters from the Queen: Fields of Massacre (10785)
 * @author malyelfik
 */
public class Q10785_LettersFromTheQueenFieldsOfMassacre extends LetterQuest
{
	// NPCs
	private static final int ORVEN = 30857;
	private static final int SHUVANN = 33867;
	
	// Items
	private static final int SOE_ADEN = 39578;
	private static final int SOE_FIELDS_OF_MASSACRE = 39579;
	
	// Location
	private static final Location TELEPORT_LOC = new Location(147446, 22761, -1984);
	
	// Misc
	private static final int MIN_LEVEL = 61;
	private static final int MAX_LEVEL = 64;
	
	public Q10785_LettersFromTheQueenFieldsOfMassacre()
	{
		super(10785);
		addTalkId(ORVEN, SHUVANN);
		setIsErtheiaQuest(true);
		setLevel(MIN_LEVEL, MAX_LEVEL);
		setStartLocation(SOE_ADEN, TELEPORT_LOC);
		setStartQuestSound("Npcdialog1.serenia_quest_7");
		registerQuestItems(SOE_ADEN, SOE_FIELDS_OF_MASSACRE);
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
			case "30857-02.html":
			case "33867-02.html":
			{
				htmltext = event;
				break;
			}
			case "30857-03.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
					giveItems(player, SOE_FIELDS_OF_MASSACRE, 1);
					htmltext = event;
				}
				break;
			}
			case "33867-03.html":
			{
				if (qs.isCond(3))
				{
					giveStoryQuestReward(npc, player);
					addExpAndSp(player, 807240, 193);
					showOnScreenMsg(player, NpcStringId.GROW_STRONGER_HERE_UNTIL_YOU_RECEIVE_THE_NEXT_LETTER_FROM_QUEEN_NAVARI_AT_LV_65, ExShowScreenMessage.TOP_CENTER, 8000);
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
			if (npc.getId() == ORVEN)
			{
				htmltext = (qs.isCond(2)) ? "30857-01.html" : "30857-04.html";
			}
			else if (qs.isCond(3))
			{
				htmltext = "33867-01.html";
			}
		}
		
		return htmltext;
	}
}
