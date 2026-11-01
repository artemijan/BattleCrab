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
package ai.areas.GardenOfGenesis;

import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * Garden Watchman AI
 * @author Gigi
 * @date 2018-08-26 - [12:27:45]
 */
public class GardenWatchman extends Script
{
	// NPCs
	private static final int GARDEN_WATCHMAN = 22952;
	private static final int GENESIS_TRAP_1 = 18985;
	private static final int GENESIS_TRAP_2 = 18986;
	
	// Skills
	private static final SkillHolder TRAP_SETUP = new SkillHolder(14418, 1);
	private static final SkillHolder HARMFUL_TRAP_1 = new SkillHolder(14075, 1);
	private static final SkillHolder HARMFUL_TRAP_2 = new SkillHolder(14076, 1);
	
	public GardenWatchman()
	{
		addSpawnId(GARDEN_WATCHMAN);
		addCreatureSeeId(GENESIS_TRAP_1, GENESIS_TRAP_2);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "SPAWN_TRAP":
			{
				if ((npc != null) && npc.isSpawned() && !npc.isInCombat())
				{
					npc.doCast(TRAP_SETUP.getSkill());
					final Npc trap = addSpawn((getRandom(10) < 5) ? GENESIS_TRAP_1 : GENESIS_TRAP_2, npc, true, 90000, false);
					trap.setDisplayEffect(1);
					startQuestTimer("SPAWN_TRAP", getRandom(50000, 100000), npc, null);
				}
				break;
			}
			case "DEBUFF":
			{
				World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 100, nearby ->
				{
					if ((npc != null) && npc.isScriptValue(0) && nearby.isPlayer() && GeoEngine.getInstance().canSeeTarget(npc, nearby))
					{
						npc.setScriptValue(1);
						npc.setTarget(nearby);
						npc.doCast((getRandom(10) < 5) ? HARMFUL_TRAP_1.getSkill() : HARMFUL_TRAP_2.getSkill());
						npc.deleteMe();
					}
				});
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		cancelQuestTimer("SPAWN_TRAP", npc, null);
		startQuestTimer("SPAWN_TRAP", 50000, npc, null);
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer())
		{
			startQuestTimer("DEBUFF", 3000, npc, null, true);
		}
	}
	
	public static void main(String[] args)
	{
		new GardenWatchman();
	}
}
