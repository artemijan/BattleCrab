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
package ai.areas.PaganTemple;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;

import ai.AbstractNpcAI;

/**
 * Custom script to remove custom key drop from NPC XMLs.<br>
 * Used to access more conveniently Pagan Temple.
 * @author Mobius
 */
public class PaganKeys extends AbstractNpcAI
{
	// Items
	private static final int ANTEROOM_KEY = 8273;
	private static final int CHAPEL_KEY = 8274;
	private static final int KEY_OF_DARKNESS = 8275;
	// NPCs
	private static final int ZOMBIE_WORKER = 22140;
	private static final int TRIOLS_LAYPERSON = 22142;
	private static final int TRIOLS_PRIEST = 22168;
	// Misc
	private static final int ANTEROOM_KEY_CHANCE = 10;
	private static final int CHAPEL_KEY_CHANCE = 10;
	private static final int KEY_OF_DARKNESS_CHANCE = 10;
	
	private PaganKeys()
	{
		addKillId(ZOMBIE_WORKER, TRIOLS_LAYPERSON, TRIOLS_PRIEST);
	}
	
	@Override
	public String onKill(Npc npc, Player killer, boolean isSummon)
	{
		switch (npc.getId())
		{
			case ZOMBIE_WORKER:
			{
				if (getRandom(100) < ANTEROOM_KEY_CHANCE)
				{
					if (Config.AUTO_LOOT)
					{
						giveItems(killer, ANTEROOM_KEY, 1);
					}
					else
					{
						npc.dropItem(killer, ANTEROOM_KEY, 1);
					}
				}
				break;
			}
			case TRIOLS_LAYPERSON:
			{
				if (getRandom(100) < CHAPEL_KEY_CHANCE)
				{
					if (Config.AUTO_LOOT)
					{
						giveItems(killer, CHAPEL_KEY, 1);
					}
					else
					{
						npc.dropItem(killer, CHAPEL_KEY, 1);
					}
				}
				break;
			}
			case TRIOLS_PRIEST:
			{
				if (getRandom(100) < KEY_OF_DARKNESS_CHANCE)
				{
					if (Config.AUTO_LOOT)
					{
						giveItems(killer, KEY_OF_DARKNESS, 1);
					}
					else
					{
						npc.dropItem(killer, KEY_OF_DARKNESS, 1);
					}
				}
				break;
			}
		}
		return super.onKill(npc, killer, isSummon);
	}
	
	public static void main(String[] args)
	{
		new PaganKeys();
	}
}
