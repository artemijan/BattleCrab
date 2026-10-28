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
package ai.others.BalthusKnights.Helper.BalthusKnightAteld;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * Balthus Knight Ateld AI
 * @author Kazumi
 */
public final class BalthusKnightAteld extends Script
{
	// NPCs
	private static final int ATELD = 34369;
	
	// Monsters
	private static final int ANTHARAS_TRANSFORM = 24088;
	
	// Skills
	// TODO: It's "Balthus Volcanic Destruction" on Orfen
	private static final SkillHolder ElementalSpikeSkill = new SkillHolder(32152, 1);
	
	// Misc
	private static final int p_CheckFirstInterval = 15000;
	private static final int p_CheckInterval = 3000;
	
	public BalthusKnightAteld()
	{
		addSpawnId(ATELD);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		
		switch (npc.getId())
		{
			case ATELD:
			{
				if ((instance != null) && (instance.getTemplateId() == 271))
				{
					final Npc antharas_trans = instance.getNpc(ANTHARAS_TRANSFORM);
					if (antharas_trans != null)
					{
						npc.setTarget(antharas_trans);
					}
					
					npc.setInvul(true);
					npc.setTalkable(false);
					npc.setRandomWalking(false);
					npc.setTargetable(false);
					ThreadPool.schedule(new CheckTask(npc, instance), p_CheckFirstInterval);
				}
				break;
			}
		}
	}
	
	private class CheckTask implements Runnable
	{
		private final Npc _npc;
		private final Instance _instance;
		
		public CheckTask(Npc npc, Instance instance)
		{
			_npc = npc;
			_instance = instance;
		}
		
		@Override
		public void run()
		{
			if ((_instance != null) && (_instance.getStatus() < 5))
			{
				if ((_npc != null) && !_npc.isDead() && !_npc.isDecayed())
				{
					ThreadPool.schedule(new CheckTask(_npc, _instance), p_CheckInterval);
					final Npc antharas_trans = _instance.getNpc(ANTHARAS_TRANSFORM);
					if (antharas_trans != null)
					{
						final double distance = _npc.calculateDistance2D(antharas_trans);
						if ((distance > 300) || (distance < 200))
						{
							_npc.setRunning();
							addMoveToDesire(_npc, antharas_trans.getPointInRange(200, 300), 23);
						}
						else
						{
							addSkillCastDesire(_npc, antharas_trans, ElementalSpikeSkill, 20000);
						}
					}
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new BalthusKnightAteld();
	}
}
