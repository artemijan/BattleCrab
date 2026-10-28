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

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * Leopard Dragon Hachling AI.
 * @author Mobius
 */
public class LeopardDragonHachling extends Script
{
	// NPCs
	private static final int DRAGON_HACHLING = 23434;
	private static final int LEOPARD_DRAGON = 23435;
	
	// Locations
	private static final List<Location> TRANSFORM_LOCATIONS = new ArrayList<>();
	static
	{
		TRANSFORM_LOCATIONS.add(new Location(84199, 120022, -2944));
		TRANSFORM_LOCATIONS.add(new Location(92138, 113735, -3076));
		TRANSFORM_LOCATIONS.add(new Location(103925, 122422, -3776));
		TRANSFORM_LOCATIONS.add(new Location(122040, 115493, -3648));
	}
	
	private LeopardDragonHachling()
	{
		addAttackId(DRAGON_HACHLING);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if ((npc != null) && event.equals("MOVE_TO_TRANSFORM"))
		{
			if (npc.calculateDistance2D(nearestLocation(npc)) < 100)
			{
				final int random = getRandom(1, 4);
				for (int counter = 1; counter < random; counter++)
				{
					final Npc leopard = addSpawn(LEOPARD_DRAGON, npc.getLocation(), true, 300000); // 5 minute despawn time
					leopard.broadcastPacket(new NpcSay(leopard.getObjectId(), ChatType.NPC_GENERAL, LEOPARD_DRAGON, NpcStringId.I_M_GOING_TO_TRANSFORM_WITH_THE_POWER_OF_THE_VORTEX_YOU_JUST_WATCH));
					addAttackDesire(leopard, player);
				}
				
				cancelQuestTimer("MOVE_TO_TRANSFORM", npc, player);
				npc.deleteMe();
			}
			else
			{
				npc.abortAttack();
				npc.getAI().setIntention(Intention.MOVE_TO, nearestLocation(npc));
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon, Skill skill)
	{
		if (npc.isScriptValue(0))
		{
			npc.setScriptValue(1);
			npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, DRAGON_HACHLING, NpcStringId.HEY_THAT_HURT_YOU_JUST_WAIT_HERE_AND_I_LL_BE_BACK_AS_A_STRONGER_DRAGON));
			startQuestTimer("MOVE_TO_TRANSFORM", 1000, npc, attacker, true);
		}
		
		npc.abortAttack();
		npc.getAI().setIntention(Intention.MOVE_TO, nearestLocation(npc));
	}
	
	private Location nearestLocation(Npc npc)
	{
		Location gotoLoc = TRANSFORM_LOCATIONS.get(0);
		for (Location loc : TRANSFORM_LOCATIONS)
		{
			if (npc.calculateDistance2D(loc) < npc.calculateDistance2D(gotoLoc))
			{
				gotoLoc = loc;
			}
		}
		
		return gotoLoc;
	}
	
	public static void main(String[] args)
	{
		new LeopardDragonHachling();
	}
}
