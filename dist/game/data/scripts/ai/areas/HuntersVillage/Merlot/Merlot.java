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
package ai.areas.HuntersVillage.Merlot;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Merlot AI.
 * @author crystalgarden
 */
public class Merlot extends Script
{
	// NPC
	private static final int MERLOT = 34018;
	
	// Item
	private static final int ATELIA_CRYSTAL = 45610;
	private static final int DIMENSIONAL_COIN = 45941;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	// Location
	private static final Location DIMENSIONAL_RAID = new Location(116503, 75392, -2712); // Merlot Position
	
	private Merlot()
	{
		addStartNpc(MERLOT);
		addTalkId(MERLOT);
		addFirstTalkId(MERLOT);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "34018-2.htm":
			case "34018-3.htm":
			{
				htmltext = event;
				break;
			}
			case "give_coin":
			{
				if (hasQuestItems(player, ATELIA_CRYSTAL))
				{
					giveItems(player, DIMENSIONAL_COIN, 1);
					addExpAndSp(player, 0, 14000000);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
				else
				{
					htmltext = "34018-5.htm";
				}
				break;
			}
			case "dimensional_raid": // Need TODO Dimensional Raid
			{
				if (player.getLevel() < MIN_LEVEL)
				{
					htmltext = "34018-1.htm";
				}
				else
				{
					player.teleToLocation(DIMENSIONAL_RAID);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "34018.htm";
	}
	
	public static void main(String[] args)
	{
		new Merlot();
	}
}
