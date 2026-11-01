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
package ai.areas.Heine;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.enums.Movie;

import quests.Q10455_ElikiasLetter.Q10455_ElikiasLetter;

/**
 * Warp Gate AI.
 * @author Gigi
 */
public class WrapGate extends Script
{
	// NPC
	private static final int WRAP_GATE = 33900;
	
	// Location
	private static final Location TELEPORT_LOC = new Location(-28575, 255984, -2195);
	
	private WrapGate()
	{
		addStartNpc(WRAP_GATE);
		addFirstTalkId(WRAP_GATE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if ("enter_hellbound".equals(event))
		{
			final QuestState qs = player.getQuestState(Q10455_ElikiasLetter.class.getSimpleName());
			if ((qs != null) && qs.isCond(1))
			{
				playMovie(player, Movie.SC_HELLBOUND);
			}
			
			player.teleToLocation(TELEPORT_LOC);
		}
		
		return null;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "33900.html";
	}
	
	public static void main(String[] args)
	{
		new WrapGate();
	}
}
