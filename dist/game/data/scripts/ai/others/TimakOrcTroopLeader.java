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
package ai.others;

import org.l2jmobius.gameserver.enums.ChatType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.holders.MinionHolder;
import org.l2jmobius.gameserver.network.NpcStringId;

import ai.AbstractNpcAI;

/**
 * Timak Orc Troop Leader AI.
 * @author Mobius
 */
public class TimakOrcTroopLeader extends AbstractNpcAI
{
	private static final int TIMAK_ORC_TROOP_LEADER = 20767;
	private static final NpcStringId[] ON_ATTACK_MSG =
	{
		NpcStringId.COME_OUT_YOU_CHILDREN_OF_DARKNESS,
		NpcStringId.SHOW_YOURSELVES,
		NpcStringId.DESTROY_THE_ENEMY_MY_BROTHERS,
		NpcStringId.FORCES_OF_DARKNESS_FOLLOW_ME
	};
	
	private TimakOrcTroopLeader()
	{
		addAttackId(TIMAK_ORC_TROOP_LEADER);
	}
	
	@Override
	public String onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if (npc.isMonster())
		{
			final Monster monster = (Monster) npc;
			if (!monster.isTeleporting() && (getRandom(1, 100) <= npc.getParameters().getInt("SummonPrivateRate", 0)) && (monster.getMinionList().countSpawnedMinions() < 3))
			{
				for (MinionHolder is : npc.getParameters().getMinionList("Privates"))
				{
					boolean found = false;
					for (Monster minion : monster.getMinionList().getSpawnedMinions())
					{
						if (minion.getId() == is.getId())
						{
							found = true;
							break;
						}
					}
					if (!found)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_ATTACK_MSG));
						addMinion(monster, is.getId());
						break;
					}
				}
			}
		}
		return super.onAttack(npc, attacker, damage, isSummon);
	}
	
	public static void main(String[] args)
	{
		new TimakOrcTroopLeader();
	}
}