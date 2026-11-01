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
package ai.areas.WindyHill;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * Wind Vortex AI (Windy Hill)
 * @author malyelfik
 */
public class WindVortex extends Script
{
	// NPC
	private static final int VORTEX = 23417;
	private static final int GIANT_WINDIMA = 23419;
	private static final int IMMENSE_WINDIMA = 23420;
	
	public WindVortex()
	{
		addAttackId(VORTEX);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if (npc.isScriptValue(0) && !npc.isDead())
		{
			npc.setScriptValue(1);
			if (attacker.getRace() == Race.ERTHEIA)
			{
				final int npcId = (attacker.isMageClass()) ? IMMENSE_WINDIMA : GIANT_WINDIMA;
				showOnScreenMsg(attacker, NpcStringId.A_POWERFUL_MONSTER_HAS_COME_TO_FACE_YOU, ExShowScreenMessage.TOP_CENTER, 5000);
				addSpawn(npcId, npc, false, 120000);
			}
		}
	}
	
	public static void main(String[] args)
	{
		new WindVortex();
	}
}
