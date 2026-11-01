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
package ai.areas.DragonValley;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

/**
 * Dragon Valley summoner NPC AI
 * @author Gigi, Mobius
 */
public class DragonValleySummoners extends Script
{
	// NPCs
	private static final int BLOODY_GRAVE_WARRIOR = 23441;
	private static final int DARK_GRAVE_WARRIOR = 23442;
	private static final int CAVE_SERVANT_ARCHER = 23436;
	private static final int CAVE_SERVANT_WARRIOR = 23437;
	
	// Config
	private static final int CHANCE = 15;
	
	private DragonValleySummoners()
	{
		addKillId(BLOODY_GRAVE_WARRIOR, DARK_GRAVE_WARRIOR, CAVE_SERVANT_ARCHER, CAVE_SERVANT_WARRIOR);
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (getRandom(100) < CHANCE)
		{
			switch (npc.getId())
			{
				case BLOODY_GRAVE_WARRIOR:
				{
					final Npc summon1 = addSpawn(BLOODY_GRAVE_WARRIOR, npc.getX() + 40, npc.getY() + 40, npc.getZ(), npc.getHeading(), false, 120000, true);
					addAttackPlayerDesire(summon1, killer);
					break;
				}
				case DARK_GRAVE_WARRIOR:
				{
					final Npc summon2 = addSpawn(DARK_GRAVE_WARRIOR, npc.getX() + 40, npc.getY() + 40, npc.getZ(), npc.getHeading(), false, 120000, true);
					addAttackPlayerDesire(summon2, killer);
					break;
				}
				case CAVE_SERVANT_ARCHER:
				{
					final Npc summon3 = addSpawn(CAVE_SERVANT_ARCHER, npc.getX() + 40, npc.getY() + 40, npc.getZ(), npc.getHeading(), false, 120000, true);
					addAttackPlayerDesire(summon3, killer);
					break;
				}
				case CAVE_SERVANT_WARRIOR:
				{
					final Npc summon4 = addSpawn(CAVE_SERVANT_WARRIOR, npc.getX() + 40, npc.getY() + 40, npc.getZ(), npc.getHeading(), false, 120000, true);
					addAttackPlayerDesire(summon4, killer);
					break;
				}
			}
			
			npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THE_DEAD_ARE_CALLING_AND_I_ANSWER);
		}
	}
	
	public static void main(String[] args)
	{
		new DragonValleySummoners();
	}
}
