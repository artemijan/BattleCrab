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
package ai.areas.Aden.Penny;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;

/**
 * Aden Faction Npc AI
 * @author NightBR
 * @date 2019-03-27
 */
public class Penny extends Script
{
	// NPC
	private static final int PENNY = 34413;
	
	// Misc
	private static final String[] RANDOM_VOICE =
	{
		"Npcdialog1.peny_ep50_greeting_7",
		"Npcdialog1.peny_ep50_greeting_8",
		"Npcdialog1.peny_ep50_greeting_9"
	};
	
	private Penny()
	{
		addStartNpc(PENNY);
		addTalkId(PENNY);
		addFirstTalkId(PENNY);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "medal":
			{
				// Take medal / Give rep?
				return null;
			}
			case "grand_medal":
			{
				// Take medal / Give rep?
				return null;
			}
		}
		
		return null;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		player.sendPacket(new PlaySound(3, RANDOM_VOICE[getRandom(3)], 0, 0, 0, 0, 0));
		return "34413.html";
	}
	
	public static void main(String[] args)
	{
		new Penny();
	}
}
