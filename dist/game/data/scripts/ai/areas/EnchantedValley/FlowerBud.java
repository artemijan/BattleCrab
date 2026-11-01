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
package ai.areas.EnchantedValley;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * AI from Flower Bud in Enchanted Valley, after kill random spawn<br>
 * [Nymph Rose (Elegant), Nymph Lily (Elegant), Nymph Tulip (Elegant), Nymph Cosmos (Elegant)]
 * @author Gigi
 */
public class FlowerBud extends Script
{
	// NPCs
	private static final int FLOWER_BUD = 19600;
	private static final List<Integer> FLOWER_SPAWNS = new ArrayList<>();
	static
	{
		FLOWER_SPAWNS.add(23582);
		FLOWER_SPAWNS.add(23583);
		FLOWER_SPAWNS.add(23584);
		FLOWER_SPAWNS.add(23585);
	}
	
	private FlowerBud()
	{
		addKillId(FLOWER_BUD);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("spawn") && npc.isDead())
		{
			final Npc elegant = addSpawn(FLOWER_SPAWNS.get(getRandom(FLOWER_SPAWNS.size())), npc, false, 120000, false);
			addAttackPlayerDesire(elegant, player);
		}
		
		return event;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		startQuestTimer("spawn", 3000, npc, killer);
	}
	
	public static void main(String[] args)
	{
		new FlowerBud();
	}
}
