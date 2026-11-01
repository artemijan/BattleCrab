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
package ai.areas.TalkingIsland.Walkers;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * @author UnAfraid, Mobius
 */
public abstract class WalkerAI extends Script
{
	protected void followNpc(Npc npc, int followedNpcId, int followingAngle, int minDistance, int maxDistance)
	{
		World.getInstance().forEachVisibleObject(npc, Npc.class, npcAround ->
		{
			if (npcAround.getId() != followedNpcId)
			{
				return;
			}
			
			final double distance = npc.calculateDistance3D(npcAround);
			if ((distance >= maxDistance) && npc.isScriptValue(0))
			{
				npc.setRunning();
				npc.setScriptValue(1);
			}
			else if ((distance <= (minDistance * 1.5)) && npc.isScriptValue(1))
			{
				npc.setWalking();
				npc.setScriptValue(0);
			}
			
			final double course = Math.toRadians(followingAngle);
			final double radian = Math.toRadians(LocationUtil.convertHeadingToDegree(npcAround.getHeading()));
			final double nRadius = npc.getCollisionRadius() + npcAround.getCollisionRadius() + minDistance;
			final int x = npcAround.getLocation().getX() + (int) (Math.cos(Math.PI + radian + course) * nRadius);
			final int y = npcAround.getLocation().getY() + (int) (Math.sin(Math.PI + radian + course) * nRadius);
			final int z = npcAround.getLocation().getZ();
			npc.getAI().moveTo(new Location(x, y, z));
		});
	}
}
