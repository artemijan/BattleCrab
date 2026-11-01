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
package ai.areas.ImperialTomb.Zenya;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Zenya AI.
 * @author Stayway
 */
public class Zenya extends Script
{
	// NPC
	private static final int ZENYA = 32140;
	
	// Location
	private static final Location IMPERIAL_TOMB = new Location(183400, -81208, -5323);
	
	// Misc
	private static final int MIN_LEVEL = 80;
	
	private Zenya()
	{
		addStartNpc(ZENYA);
		addFirstTalkId(ZENYA);
		addTalkId(ZENYA);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "32140.html":
			case "32140-1.html":
			case "32140-2.html":
			case "32140-4.html":
			{
				htmltext = event;
				break;
			}
			case "teleport":
			{
				player.teleToLocation(IMPERIAL_TOMB);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return player.getLevel() < MIN_LEVEL ? "32140-3.html" : "32140.html";
	}
	
	public static void main(String[] args)
	{
		new Zenya();
	}
}
