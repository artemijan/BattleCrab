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
package custom.NoblessMaster;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.enums.QuestSound;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;

import ai.AbstractNpcAI;

/**
 * @author Mobius
 */
public class NoblessMaster extends AbstractNpcAI
{
	// Item
	private static final int NOBLESS_TIARA = 7694;
	
	private NoblessMaster()
	{
		addStartNpc(Config.NOBLESS_MASTER_NPCID);
		addTalkId(Config.NOBLESS_MASTER_NPCID);
		addFirstTalkId(Config.NOBLESS_MASTER_NPCID);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (!Config.NOBLESS_MASTER_ENABLED)
		{
			return null;
		}
		
		switch (event)
		{
			case "noblesse":
			{
				if (player.isNoble())
				{
					return "1003000-3.htm";
				}
				if (player.getLevel() >= Config.NOBLESS_MASTER_LEVEL_REQUIREMENT)
				{
					if (Config.NOBLESS_MASTER_REWARD_TIARA)
					{
						giveItems(player, NOBLESS_TIARA, 1);
					}
					player.setNoble(true);
					player.sendPacket(QuestSound.ITEMSOUND_QUEST_FINISH.getPacket());
					return "1003000-1.htm";
				}
				return "1003000-2.htm";
			}
		}
		
		return null;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "1003000.htm";
	}
	
	public static void main(String[] args)
	{
		new NoblessMaster();
	}
}
