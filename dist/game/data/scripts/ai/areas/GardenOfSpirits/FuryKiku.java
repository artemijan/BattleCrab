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
package ai.areas.GardenOfSpirits;

import org.l2jmobius.gameserver.ai.Action;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Fury Kiku AI
 * @author Gigi
 * @date 2018-07-23 - [15:47:15]
 */
public class FuryKiku extends Script
{
	// Monsters
	private static final int FURYKIKU = 23545;
	private static final int[] MONSTERS =
	{
		23544, // Fury Sylph Barrena
		23553, // Fury Sylph Barrena (night)
	};
	
	public FuryKiku()
	{
		addKillId(MONSTERS);
		addSpawnId(FURYKIKU);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "SPAWN":
			{
				final Party party = player.getParty();
				if (party != null)
				{
					party.getMembers().forEach(p -> addSpawn(FURYKIKU, p, true, 180000, true, 0));
				}
				else
				{
					addSpawn(FURYKIKU, player, true, 180000, true, 0);
				}
				break;
			}
			case "ATTACK":
			{
				npc.setRunning();
				World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 300, p ->
				{
					if ((p != null) && p.isPlayable() && !p.isDead())
					{
						npc.getAI().notifyAction(Action.AGGRESSION, p, 1000);
					}
				});
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (getRandom(10) < 5)
		{
			startQuestTimer("SPAWN", 2000, npc, killer);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("ATTACK", 1000, npc, null);
	}
	
	public static void main(String[] args)
	{
		new FuryKiku();
	}
}
