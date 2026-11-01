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
package ai.areas.FantasyIsle;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.krateisCube.KrateiArena;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Kratei's Cube AI
 * @author Mobius
 */
public class KrateisCube extends Script
{
	// NPCs
	private static final int[] MONSTERS =
	{
		18579,
		18580,
		18581,
		18582,
		18583,
		18584,
		18585,
		18586,
		18587,
		18588,
		18589,
		18590,
		18591,
		18592,
		18593,
		18594,
		18595,
		18596,
		18597,
		18598,
		18599,
		18600,
	};
	
	public KrateisCube()
	{
		addKillId(MONSTERS);
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		if (player != null)
		{
			final KrateiArena arena = player.getKrateiArena();
			if (arena != null)
			{
				arena.addPoints(player, false);
			}
		}
	}
	
	public static void main(String[] args)
	{
		new KrateisCube();
	}
}
