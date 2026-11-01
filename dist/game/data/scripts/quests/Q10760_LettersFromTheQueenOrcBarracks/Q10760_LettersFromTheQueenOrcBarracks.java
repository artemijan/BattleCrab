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
package quests.Q10760_LettersFromTheQueenOrcBarracks;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.LetterQuest;

/**
 * Letters from the Queen: Orc Barracks (10760)
 * @author malyelfik
 */
public class Q10760_LettersFromTheQueenOrcBarracks extends LetterQuest
{
	// NPC
	private static final int LEVIAN = 30037;
	private static final int PIOTUR = 30597;
	
	// Items
	private static final int SOE_GLUDIN_VILLAGE = 39486;
	private static final int SOE_ORC_BARRACKS = 39487;
	
	// Location
	private static final Location TELEPORT_LOC = new Location(-79816, 150828, -3040);
	
	// Misc
	private static final int MIN_LEVEL = 30;
	private static final int MAX_LEVEL = 39;
	
	public Q10760_LettersFromTheQueenOrcBarracks()
	{
		super(10760);
		addTalkId(LEVIAN, PIOTUR);
		setIsErtheiaQuest(true);
		setLevel(MIN_LEVEL, MAX_LEVEL);
		setStartLocation(SOE_GLUDIN_VILLAGE, TELEPORT_LOC);
		setStartQuestSound("Npcdialog1.serenia_quest_2");
		registerQuestItems(SOE_GLUDIN_VILLAGE, SOE_ORC_BARRACKS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		final String htmltext = event;
		switch (event)
		{
			case "30037-02.html":
			case "30597-02.html":
			{
				break;
			}
			case "30037-03.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
					giveItems(player, SOE_ORC_BARRACKS, 1);
					showOnScreenMsg(player, NpcStringId.TRY_USING_THE_TELEPORT_SCROLL_LEVIAN_GAVE_YOU_TO_GO_TO_ORC_BARRACKS, ExShowScreenMessage.TOP_CENTER, 5000);
				}
				break;
			}
			case "30597-03.html":
			{
				if (qs.isCond(3))
				{
					giveStoryQuestReward(npc, player);
					addExpAndSp(player, 242760, 58);
					showOnScreenMsg(player, NpcStringId.TRY_TALKING_TO_VORBOS_BY_THE_WELL_NYOU_CAN_RECEIVE_QUEEN_NAVARI_S_NEXT_LETTER_AT_LV_40, ExShowScreenMessage.TOP_CENTER, 8000);
					qs.exitQuest(false, true);
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
			if ((npc.getId() == LEVIAN))
			{
				htmltext = (qs.isCond(2)) ? "30037-01.html" : "30037-04.html";
			}
			else if (qs.isCond(3))
			{
				htmltext = "30597-01.html";
			}
		}
		
		return htmltext;
	}
}
