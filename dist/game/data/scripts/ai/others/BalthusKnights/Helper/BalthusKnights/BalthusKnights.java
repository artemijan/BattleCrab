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
package ai.others.BalthusKnights.Helper.BalthusKnights;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Balthus Knights AI
 * @author Kazumi
 */
public final class BalthusKnights extends Script
{
	// NPCs
	private static final int BALTHUS_KNIGHT = 34372;
	
	// Monsters
	private static final int HATCHLING = 24090;
	private static final int GEM_DRAGON_ANTHARAS = 24091;
	
	public BalthusKnights()
	{
		addSpawnId(BALTHUS_KNIGHT);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		
		if ((instance != null) && (instance.getTemplateId() == 271))
		{
			switch (instance.getStatus())
			{
				case 0:
				{
					npc.setInvul(true);
					addSpawn((Rnd.get(2) == 1) ? GEM_DRAGON_ANTHARAS : HATCHLING, npc.getX(), npc.getY(), npc.getZ(), 0, true, 0, false, instance.getId());
					ThreadPool.schedule(() ->
					{
						World.getInstance().forEachVisibleObjectInRange(npc, Monster.class, 200, mob ->
						{
							if ((mob != null) && (!mob.isDead()) && ((mob.getId() == HATCHLING) || (mob.getId() == GEM_DRAGON_ANTHARAS)))
							{
								npc.asAttackable().addDamageHate(mob, 0, 999);
								addAttackDesire(npc, mob);
							}
						});
					}, 2000); // 2 sec
					break;
				}
				case 3:
				{
					npc.setInvul(true);
					ThreadPool.schedule(() ->
					{
						World.getInstance().forEachVisibleObjectInRange(npc, Monster.class, 60, mob ->
						{
							if ((mob != null) && (!mob.isDead()) && ((mob.getId() == HATCHLING) || (mob.getId() == GEM_DRAGON_ANTHARAS)))
							{
								npc.asAttackable().addDamageHate(mob, 0, 999);
								addAttackDesire(npc, mob);
							}
						});
					}, 2000); // 2 sec
					break;
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new BalthusKnights();
	}
}
