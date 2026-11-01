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
package events.RedLibra;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.LongTimeEvent;

/**
 * Red Libra<br>
 * Info - http://www.lineage2.com/en/news/events/01202016-red-libra.php
 * @author Mobius
 */
public class RedLibra extends LongTimeEvent
{
	// NPCs
	private static final int RED = 34210;
	private static final int GREEN = 34211;
	private static final int BLACK = 34212;
	private static final int PINK = 34213;
	private static final int BLUE = 34214;
	
	private RedLibra()
	{
		addStartNpc(RED, GREEN, BLACK, PINK, BLUE);
		addFirstTalkId(RED, GREEN, BLACK, PINK, BLUE);
		addTalkId(RED, GREEN, BLACK, PINK, BLUE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "34210-1.htm":
			case "34211-1.htm":
			case "34211-2.htm":
			case "34212-1.htm":
			case "34212-2.htm":
			case "34212-3.htm":
			case "34213-1.htm":
			case "34213-2.htm":
			case "34213-3.htm":
			case "34214-1.htm":
			{
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + "-1.htm";
	}
	
	public static void main(String[] args)
	{
		new RedLibra();
	}
}
