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
package ai.areas.GardenOfSpirits.Belas;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Teleporter Belas AI
 * @author Gigi
 */
public class Belas extends Script
{
	// NPC
	private static final int BELAS = 34056;
	
	// Teleports
	private static final Location EAST = new Location(-41168, 79507, -4000);
	private static final Location WEST = new Location(-59485, 79782, -4104);
	
	// Item
	private static final int MARK_OF_TRUST_MID_GRADE = 45843;
	private static final int MARK_OF_TRUST_HIGH_GRADE = 45848;
	
	private Belas()
	{
		addFirstTalkId(BELAS);
		addTalkId(BELAS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "34056-01.html":
			{
				htmltext = event;
				break;
			}
			case "West":
			{
				if (hasQuestItems(player, MARK_OF_TRUST_MID_GRADE) || hasQuestItems(player, MARK_OF_TRUST_HIGH_GRADE))
				{
					player.teleToLocation(WEST);
					break;
				}
				
				htmltext = "34056-02.html";
				break;
			}
			case "East":
			{
				if (hasQuestItems(player, MARK_OF_TRUST_MID_GRADE) || hasQuestItems(player, MARK_OF_TRUST_HIGH_GRADE))
				{
					player.teleToLocation(EAST);
					break;
				}
				
				htmltext = "34056-02.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "34056.html";
	}
	
	public static void main(String[] args)
	{
		new Belas();
	}
}
