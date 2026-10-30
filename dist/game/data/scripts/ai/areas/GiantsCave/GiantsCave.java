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
package ai.areas.GiantsCave;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;

/**
 * Giants Cave Npcs AI
 * @author Gigi
 * @date 2017-03-04 - [17:33:25]
 */
public class GiantsCave extends Script
{
	// NPC
	private static final int SUMADRIBA = 34217;
	private static final int KRENAHT = 34237;
	private static final int STHOR = 34219;
	private static final int GIANT_TELEPORT = 34223;
	
	private GiantsCave()
	{
		addTalkId(SUMADRIBA, KRENAHT, GIANT_TELEPORT, STHOR);
		addFirstTalkId(SUMADRIBA, KRENAHT, GIANT_TELEPORT, STHOR);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "34237.html":
			case "34237-1.html":
			case "34237-2.html":
			case "34237-3.html":
			case "34237-4.html":
			case "34237-5.html":
			case "34237-6.html":
			{
				htmltext = event;
				break;
			}
			case "first_area":
			{
				htmltext = "34223-1.html";
				break;
			}
			case "second_area":
			{
				htmltext = "34223-1.html";
				break;
			}
			case "third_area":
			{
				htmltext = "34223-1.html";
				break;
			}
			case "stronghold":
			{
				htmltext = "34223-2.html";
				break;
			}
			case "relics_rooom":
			{
				htmltext = "34223-3.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		switch (npc.getId())
		{
			case SUMADRIBA:
			{
				player.sendPacket(new PlaySound(3, "Npcdialog1.schmadriba_faction_1", 0, 0, 0, 0, 0));
				break;
			}
			case KRENAHT:
			{
				if (getRandom(10) < 5)
				{
					player.sendPacket(new PlaySound(3, "Npcdialog1.krenat_faction_2", 0, 0, 0, 0, 0));
				}
				else
				{
					player.sendPacket(new PlaySound(3, "Npcdialog1.krenat_faction_1", 0, 0, 0, 0, 0));
				}
				break;
			}
			case STHOR:
			{
				if (getRandom(10) < 4)
				{
					player.sendPacket(new PlaySound(3, "Npcdialog1.stor_faction_2", 0, 0, 0, 0, 0));
				}
				else
				{
					player.sendPacket(new PlaySound(3, "Npcdialog1.stor_faction_1", 0, 0, 0, 0, 0));
				}
				break;
			}
		}
		
		return npc.getId() + ".html";
	}
	
	public static void main(String[] args)
	{
		new GiantsCave();
	}
}
