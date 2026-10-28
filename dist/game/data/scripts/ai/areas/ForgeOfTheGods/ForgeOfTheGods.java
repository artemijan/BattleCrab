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
package ai.areas.ForgeOfTheGods;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Forge of the Gods AI
 * @author nonom, malyelfik
 */
public class ForgeOfTheGods extends Script
{
	// NPCs
	private static final int[] FOG_MOBS =
	{
		22634, // Scarlet Stakato Worker
		22635, // Scarlet Stakato Soldier
		22636, // Scarlet Stakato Noble
		22637, // Tepra Scorpion
		22638, // Tepra Scarab
		22639, // Assassin Beetle
		22640, // Mercenary of Destruction
		22641, // Knight of Destruction
		22642, // Lavastone Golem
		22643, // Magma Golem
		22644, // Arimanes of Destruction
		22645, // Balor of Destruction
		22646, // Ashuras of Destruction
		22647, // Lavasillisk
		22648, // Blazing Ifrit
		22649, // Magma Drake
	};
	
	private static final int[] LAVASAURUSES =
	{
		18799, // Newborn Lavasaurus
		18800, // Fledgling Lavasaurus
		18801, // Adult Lavasaurus
		18802, // Elderly Lavasaurus
		18803, // Ancient Lavasaurus
	};
	
	private static final int REFRESH = 15;
	
	private static final int MOBCOUNT_BONUS_MIN = 3;
	
	private static final int BONUS_UPPER_LV01 = 5;
	private static final int BONUS_UPPER_LV02 = 10;
	private static final int BONUS_UPPER_LV03 = 15;
	private static final int BONUS_UPPER_LV04 = 20;
	private static final int BONUS_UPPER_LV05 = 35;
	
	private static final int BONUS_LOWER_LV01 = 5;
	private static final int BONUS_LOWER_LV02 = 10;
	private static final int BONUS_LOWER_LV03 = 15;
	
	private static final int FORGE_BONUS01 = 20;
	private static final int FORGE_BONUS02 = 40;
	
	private static int _npcCount = 0;
	
	// private static int _npcsAlive = 0; TODO: Require zone spawn support
	
	private ForgeOfTheGods()
	{
		addKillId(FOG_MOBS);
		addSpawnId(LAVASAURUSES);
		startQuestTimer("refresh", REFRESH * 1000, null, null, true);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "suicide":
			{
				if (npc != null)
				{
					npc.doDie(null);
				}
				break;
			}
			case "refresh":
			{
				_npcCount = 0;
				break;
			}
		}
		
		return null;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final int rand = getRandom(100);
		Npc mob = null;
		_npcCount++;
		
		// For monsters at Forge of the Gods - Lower level
		if (npc.getSpawn().getZ() < -5000) // && (_npcsAlive < 48))
		{
			if ((_npcCount > BONUS_LOWER_LV03) && (rand <= FORGE_BONUS02))
			{
				mob = addSpawn(LAVASAURUSES[4], npc, true);
			}
			else if (_npcCount > BONUS_LOWER_LV02)
			{
				mob = spawnLavasaurus(npc, rand, LAVASAURUSES[4], LAVASAURUSES[3]);
			}
			else if (_npcCount > BONUS_LOWER_LV01)
			{
				mob = spawnLavasaurus(npc, rand, LAVASAURUSES[3], LAVASAURUSES[2]);
			}
			else if (_npcCount >= MOBCOUNT_BONUS_MIN)
			{
				mob = spawnLavasaurus(npc, rand, LAVASAURUSES[2], LAVASAURUSES[1]);
			}
		}
		else
		// if (_npcsAlive < 32)
		{
			if ((_npcCount > BONUS_UPPER_LV05) && (rand <= FORGE_BONUS02))
			{
				mob = addSpawn(LAVASAURUSES[1], npc, true);
			}
			else if (_npcCount > BONUS_UPPER_LV04)
			{
				mob = spawnLavasaurus(npc, rand, LAVASAURUSES[4], LAVASAURUSES[3]);
			}
			else if (_npcCount > BONUS_UPPER_LV03)
			{
				mob = spawnLavasaurus(npc, rand, LAVASAURUSES[3], LAVASAURUSES[2]);
			}
			else if (_npcCount > BONUS_UPPER_LV02)
			{
				mob = spawnLavasaurus(npc, rand, LAVASAURUSES[2], LAVASAURUSES[1]);
			}
			else if (_npcCount > BONUS_UPPER_LV01)
			{
				mob = spawnLavasaurus(npc, rand, LAVASAURUSES[1], LAVASAURUSES[0]);
			}
			else if ((_npcCount >= MOBCOUNT_BONUS_MIN) && (rand <= FORGE_BONUS01))
			{
				mob = addSpawn(LAVASAURUSES[0], npc, true);
			}
		}
		
		if (mob != null)
		{
			mob.asAttackable().addDamageHate(killer, 0, 9999);
			mob.getAI().setIntention(Intention.ATTACK);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("suicide", 60000, npc, null);
	}
	
	private Npc spawnLavasaurus(Npc npc, int rand, int... mobs)
	{
		if (mobs.length < 2)
		{
			return null;
		}
		
		Npc mob = null;
		if (rand <= FORGE_BONUS01)
		{
			mob = addSpawn(mobs[0], npc, true);
		}
		else if (rand <= FORGE_BONUS02)
		{
			mob = addSpawn(mobs[1], npc, true);
		}
		
		return mob;
	}
	
	public static void main(String[] args)
	{
		new ForgeOfTheGods();
	}
}
