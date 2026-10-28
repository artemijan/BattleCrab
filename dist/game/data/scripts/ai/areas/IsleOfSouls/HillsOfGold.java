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
package ai.areas.IsleOfSouls;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Hills of Gold AI.
 * @author Mobius
 */
public class HillsOfGold extends Script
{
	// NPCs
	private static final int GOLEM_OF_REPAIRS = 19309;
	private static final int EXCAVATOR_GOLEM = 19312;
	private static final int DRILL_GOLEM = 19310;
	private static final int SPICULA_1 = 23246;
	private static final int SPICULA_2 = 23247;
	private static final int YIN_FRAGMENT = 19308;
	private static final int SPICULA_ELITE_GUARD = 23303;
	private static final int[] GOLEMS =
	{
		23255,
		23257,
		23259,
		23261,
		23263,
		23264,
		23266,
		23267,
	};
	
	public HillsOfGold()
	{
		addAttackId(YIN_FRAGMENT);
		addSpawnId(SPICULA_1, SPICULA_2);
		addSpawnId(GOLEMS);
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		if ((npc != null) && !npc.isDead() && npc.isSpawned())
		{
			if (!npc.isInCombat())
			{
				World.getInstance().forEachVisibleObjectInRange(npc, Monster.class, npc.getAggroRange(), nearby ->
				{
					if ((nearby.getId() == GOLEM_OF_REPAIRS) || (nearby.getId() == EXCAVATOR_GOLEM) || (nearby.getId() == DRILL_GOLEM))
					{
						addAttackDesire(npc, nearby);
						return;
					}
				});
			}
			
			getTimers().addTimer("SPICULA_AGGRO_" + npc.getObjectId(), null, 10000, npc, null);
		}
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		final Npc mob1 = addSpawn(SPICULA_ELITE_GUARD, npc.getX(), npc.getY(), npc.getZ(), attacker.getHeading() + 32500, true, npc.getSpawn().getRespawnDelay());
		addAttackDesire(mob1, attacker);
		final Npc mob2 = addSpawn(SPICULA_ELITE_GUARD, npc.getX(), npc.getY(), npc.getZ(), attacker.getHeading() + 32500, true, npc.getSpawn().getRespawnDelay());
		addAttackDesire(mob2, attacker);
		final Npc mob3 = addSpawn(SPICULA_ELITE_GUARD, npc.getX(), npc.getY(), npc.getZ(), attacker.getHeading() + 32500, true, npc.getSpawn().getRespawnDelay());
		addAttackDesire(mob3, attacker);
		final Npc mob4 = addSpawn(SPICULA_ELITE_GUARD, npc.getX(), npc.getY(), npc.getZ(), attacker.getHeading() + 32500, true, npc.getSpawn().getRespawnDelay());
		addAttackDesire(mob4, attacker);
		npc.deleteMe();
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		if ((npc.getId() == SPICULA_1) || (npc.getId() == SPICULA_2))
		{
			getTimers().addTimer("SPICULA_AGGRO_" + npc.getObjectId(), null, 5000, npc, null);
		}
		else
		{
			npc.setDisplayEffect(1);
		}
	}
	
	public static void main(String[] args)
	{
		new HillsOfGold();
	}
}
