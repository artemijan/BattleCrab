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
package ai.areas.Aden.Ruine;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Ruine AI
 * @author Gigi
 * @date 2017-02-18 - [20:14:22]
 */
public class Ruine extends Script
{
	// NPC
	private static final int COD_ADEN_OFFICER = 34229;
	
	// Level checks
	private static final int MIN_LEVEL_CRACK = 95;
	private static final int MIN_LEVEL_RIFT = 100;
	
	// Teleports
	private static final Location DIMENSIONAL_CRACK = new Location(-119304, -182456, -6752);
	private static final Location DIMENSIONAL_RIFT = new Location(140629, 79672, -5424);
	
	private Ruine()
	{
		addStartNpc(COD_ADEN_OFFICER);
		addFirstTalkId(COD_ADEN_OFFICER);
		addTalkId(COD_ADEN_OFFICER);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "cod_aden_officer001.htm":
			case "cod_aden_officer004.htm":
			case "cod_aden_officer005.htm":
			{
				htmltext = event;
				break;
			}
			case "crack_teleport":
			{
				if (player.getLevel() >= MIN_LEVEL_CRACK)
				{
					player.teleToLocation(DIMENSIONAL_CRACK);
					break;
				}
				
				htmltext = "cod_aden_officer003.htm";
				break;
			}
			case "rift_teleport":
			{
				if (player.getLevel() >= MIN_LEVEL_RIFT)
				{
					player.teleToLocation(DIMENSIONAL_RIFT);
					break;
				}
				
				htmltext = "cod_aden_officer003.htm";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "cod_aden_officer001.htm";
	}
	
	public static void main(String[] args)
	{
		new Ruine();
	}
}
