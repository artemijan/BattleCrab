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
package ai.areas.WharfGludioAirships.ZealotOfShilen;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Zealot of Shilen AI.
 * @author nonom, Mobius
 */
public class ZealotOfShilen extends Script
{
	// NPCs
	private static final int ZEALOT = 18782;
	private static final int[] GUARDS =
	{
		32628,
		32629
	};
	
	public ZealotOfShilen()
	{
		addSpawnId(ZEALOT);
		addSpawnId(GUARDS);
		addFirstTalkId(GUARDS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (npc == null)
		{
			return null;
		}
		
		if (!npc.isAttackingNow() && !npc.isAlikeDead())
		{
			Npc nearby = null;
			double maxDistance = Double.MAX_VALUE;
			for (Monster obj : World.getInstance().getVisibleObjects(npc, Monster.class))
			{
				final double distance = npc.calculateDistance2D(obj);
				if ((distance < maxDistance) && !obj.isDead() && !obj.isDecayed())
				{
					maxDistance = distance;
					nearby = obj;
				}
			}
			
			if (nearby != null)
			{
				npc.setRunning();
				npc.asAttackable().addDamageHate(nearby, 0, 999);
				npc.getAI().setIntention(Intention.ATTACK, nearby, null);
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return (npc.isAttackingNow()) ? "32628-01.html" : npc.getId() + ".html";
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		if (npc.getId() == ZEALOT)
		{
			npc.setRandomWalking(false);
		}
		else
		{
			npc.setInvul(true);
			npc.asAttackable().setCanReturnToSpawnPoint(false);
			cancelQuestTimer("WATCHING", npc, null);
			startQuestTimer("WATCHING", 10000, npc, null, true);
		}
	}
	
	public static void main(String[] args)
	{
		new ZealotOfShilen();
	}
}
