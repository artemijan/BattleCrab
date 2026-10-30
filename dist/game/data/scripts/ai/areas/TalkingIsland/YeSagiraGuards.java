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
package ai.areas.TalkingIsland;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Ye Sagira Guards AI.
 * @author Mobius
 */
public class YeSagiraGuards extends Script
{
	// NPCs
	private static final int[] GUARDS =
	{
		19152,
		19153
	};
	
	private YeSagiraGuards()
	{
		addSpawnId(GUARDS);
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		if ((npc != null) && !npc.isDead() && npc.isSpawned())
		{
			if (!npc.isInCombat())
			{
				final Monster monster = getRandomEntry(World.getInstance().getVisibleObjectsInRange(npc, Monster.class, 1000));
				if ((monster != null) && !monster.isDead() && !monster.isInCombat())
				{
					npc.reduceCurrentHp(1, monster, null); // TODO: Find better way for attack
				}
			}
			
			getTimers().addTimer("GUARD_AGGRO_" + npc.getObjectId(), null, 10000, npc, null);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.setInvul(true);
		getTimers().addTimer("GUARD_AGGRO_" + npc.getObjectId(), null, 5000, npc, null);
	}
	
	public static void main(String[] args)
	{
		new YeSagiraGuards();
	}
}
