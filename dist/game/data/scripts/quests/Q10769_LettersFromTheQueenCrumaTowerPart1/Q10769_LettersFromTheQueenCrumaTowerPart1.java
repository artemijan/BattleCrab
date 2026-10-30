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
package quests.Q10769_LettersFromTheQueenCrumaTowerPart1;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.LetterQuest;

/**
 * Letters from the Queen: Cruma Tower, Part 1 (10769)
 * @author malyelfik
 */
public class Q10769_LettersFromTheQueenCrumaTowerPart1 extends LetterQuest
{
	// NPCs
	private static final int SYLVAIN = 30070;
	private static final int LORAIN = 30673;
	
	// Items
	private static final int SOE_DION_TOWN = 39593;
	private static final int SOE_CRUMA_TOWER = 39594;
	private static final int ENCHANT_WEAPON_C = 951;
	private static final int ENCHANT_ARMOR_C = 952;
	
	// Location
	private static final Location TELEPORT_LOC = new Location(16014, 142326, -2688);
	
	// Misc
	private static final int MIN_LEVEL = 40;
	private static final int MAX_LEVEL = 45;
	
	public Q10769_LettersFromTheQueenCrumaTowerPart1()
	{
		super(10769);
		addTalkId(SYLVAIN, LORAIN);
		setIsErtheiaQuest(true);
		setLevel(MIN_LEVEL, MAX_LEVEL);
		setStartLocation(SOE_DION_TOWN, TELEPORT_LOC);
		setStartQuestSound("Npcdialog1.serenia_quest_3");
		registerQuestItems(SOE_DION_TOWN, SOE_CRUMA_TOWER);
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
			case "30070-02.html":
			case "30673-02.html":
			{
				break;
			}
			case "30070-03.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
					giveItems(player, SOE_CRUMA_TOWER, 1);
					showOnScreenMsg(player, NpcStringId.TRY_USING_THE_TELEPORT_SCROLL_SYLVAIN_GAVE_YOU_TO_GO_TO_CRUMA_TOWER, ExShowScreenMessage.TOP_CENTER, 5000);
				}
				break;
			}
			case "30673-03.html":
			{
				if (qs.isCond(3))
				{
					giveItems(player, ENCHANT_WEAPON_C, 1);
					giveItems(player, ENCHANT_ARMOR_C, 1);
					giveStoryQuestReward(npc, player);
					addExpAndSp(player, 370440, 88);
					showOnScreenMsg(player, NpcStringId.GROW_STRONGER_HERE_UNTIL_YOU_RECEIVE_THE_NEXT_LETTER_FROM_QUEEN_NAVARI_AT_LV_46, ExShowScreenMessage.TOP_CENTER, 8000);
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
			if (npc.getId() == SYLVAIN)
			{
				htmltext = (qs.isCond(2)) ? "30070-01.html" : "30070-04.html";
			}
			else if (qs.isCond(3))
			{
				htmltext = "30673-01.html";
			}
		}
		
		return htmltext;
	}
}
