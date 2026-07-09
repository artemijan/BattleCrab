/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package ai.others.Spawns;

import org.l2jmobius.gameserver.model.Spawn;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.spawns.SpawnGroup;
import org.l2jmobius.gameserver.model.spawns.SpawnTemplate;

import ai.AbstractNpcAI;

/**
 * @author UnAfraid
 */
public class NoRandomActivity extends AbstractNpcAI
{
	private NoRandomActivity()
	{
	}
	
	@Override
	public void onSpawnNpc(SpawnTemplate template, SpawnGroup group, Npc npc)
	{
		final StatSet parameters = template.getParameters();
		if (parameters == null)
		{
			return;
		}
		
		npc.setRandomAnimation(!parameters.getBoolean("disableRandomAnimation", false));
		final boolean randomWalk = !parameters.getBoolean("disableRandomWalk", false);
		npc.setRandomWalking(randomWalk);
		
		final Spawn spawn = npc.getSpawn();
		if (spawn == null)
		{
			return;
		}
		
		spawn.setRandomWalking(randomWalk);
	}
	
	public static void main(String[] args)
	{
		new NoRandomActivity();
	}
}
