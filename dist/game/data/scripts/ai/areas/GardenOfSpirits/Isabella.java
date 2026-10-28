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

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Isabella Raid Boss AI
 * @URL https://www.youtube.com/watch?v=3M73b6Kre6Y
 * @author Gigi, Mobius
 */
public class Isabella extends Script
{
	// NPC
	private static final int ISABELLA = 26131;
	private static final int CROA = 26132;
	private static final int AMIS = 26133;
	private static final int CROAMIS = 23563;
	
	// Doors
	private static final int DOOR1 = 18200001;
	private static final int DOOR2 = 18200002;
	
	// Location
	private static final Location CROAMIS_SPAWN_LOCATION = new Location(-51033, 82405, -4882, 44107);
	
	public Isabella()
	{
		addAttackId(ISABELLA);
		addSpawnId(ISABELLA, CROAMIS);
		addKillId(ISABELLA, CROAMIS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("SPAWN"))
		{
			final Npc minion1 = addSpawn(CROA, -51104, 83436, -5120, 61567, false, 300000, false);
			addAttackPlayerDesire(minion1, player);
			final Npc minion2 = addSpawn(AMIS, -50307, 83662, -5120, 45183, false, 300000, false);
			addAttackPlayerDesire(minion2, player);
			final Npc minion3 = addSpawn(CROA, -50259, 82825, -5120, 23684, false, 300000, false);
			addAttackPlayerDesire(minion3, player);
			final Npc minion4 = addSpawn(AMIS, -50183, 82901, -5120, 28180, false, 300000, false);
			addAttackPlayerDesire(minion4, player);
			final Npc minion5 = addSpawn(CROA, -50387, 83732, -5112, 45183, false, 300000, false);
			addAttackPlayerDesire(minion5, player);
			final Npc minion6 = addSpawn(AMIS, -51157, 83298, -5112, 64987, true, 300000, false);
			addAttackPlayerDesire(minion6, player);
		}
		
		return null;
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if (!npc.isDead())
		{
			if ((npc.getCurrentHpPercent() <= 80) && npc.isScriptValue(0))
			{
				startQuestTimer("SPAWN", 500, npc, attacker);
				npc.setScriptValue(1);
			}
			else if ((npc.getCurrentHpPercent() <= 50) && npc.isScriptValue(1))
			{
				startQuestTimer("SPAWN", 500, npc, attacker);
				npc.setScriptValue(2);
			}
			else if ((npc.getCurrentHpPercent() <= 10) && npc.isScriptValue(2))
			{
				startQuestTimer("SPAWN", 500, npc, attacker);
				npc.setScriptValue(3);
			}
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		if (npc.getId() == ISABELLA)
		{
			addSpawn(CROAMIS, CROAMIS_SPAWN_LOCATION, false, 0);
		}
		else // CROAMIS
		{
			npc.setRandomWalking(false);
			npc.teleToLocation(CROAMIS_SPAWN_LOCATION); // No random spawn.
			closeDoor(DOOR1, 0);
			closeDoor(DOOR2, 0);
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (npc.getId() == ISABELLA)
		{
			World.getInstance().forEachVisibleObjectInRange(npc, Monster.class, 1500, minion ->
			{
				if ((minion != null) && !minion.isAlikeDead() && ((minion.getId() == CROA) || (minion.getId() == AMIS)))
				{
					minion.deleteMe();
				}
			});
		}
		else // CROAMIS
		{
			openDoor(DOOR1, 0);
			openDoor(DOOR2, 0);
		}
	}
	
	public static void main(String[] args)
	{
		new Isabella();
	}
}
