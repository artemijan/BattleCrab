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
package ai.areas.Aden.Herphah;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Aden Faction Npc AI
 * @author NightBR
 * @date 2019-03-27
 */
public class Herphah extends Script
{
	// NPC
	private static final int HERPHAH = 34362;
	
	// Misc
	@SuppressWarnings("unused")
	private static final String[] RANDOM_VOICE =
	{
		"Npcdialog1.herphah_ep50_greeting_1",
		"Npcdialog1.herphah_ep50_greeting_2",
		"Npcdialog1.herphah_ep50_greeting_3"
	};
	
	private Herphah()
	{
		addStartNpc(HERPHAH);
		addTalkId(HERPHAH);
		addFirstTalkId(HERPHAH);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "34362-01.html":
			case "34362-02.html":
			case "34362-03.html":
			case "34362-04.html":
			case "34362-05.html":
			case "34362-06.html":
			case "34362-07.html":
			case "34362-08.html":
			case "34362-09.html":
			case "34362-10.html":
			{
				return event;
			}
			default:
			{
				return null;
			}
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		// Chance to broadcast at nearby players?
		// player.sendPacket(new PlaySound(RANDOM_VOICE[getRandom(3)]));
		return "34362.html";
	}
	
	public static void main(String[] args)
	{
		new Herphah();
	}
}
