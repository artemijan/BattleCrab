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
package custom.DelevelManager;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.data.xml.ExperienceData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;

import ai.AbstractNpcAI;

/**
 * @author Mobius
 */
public class DelevelManager extends AbstractNpcAI
{
	private DelevelManager()
	{
		addStartNpc(Config.DELEVEL_MANAGER_NPCID);
		addTalkId(Config.DELEVEL_MANAGER_NPCID);
		addFirstTalkId(Config.DELEVEL_MANAGER_NPCID);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (!Config.DELEVEL_MANAGER_ENABLED)
		{
			return null;
		}
		
		switch (event)
		{
			case "delevel":
			{
				if (player.getLevel() <= Config.DELEVEL_MANAGER_MINIMUM_DELEVEL)
				{
					return "1002000-2.htm";
				}
				if (getQuestItemsCount(player, Config.DELEVEL_MANAGER_ITEMID) >= Config.DELEVEL_MANAGER_ITEMCOUNT)
				{
					takeItems(player, Config.DELEVEL_MANAGER_ITEMID, Config.DELEVEL_MANAGER_ITEMCOUNT);
					player.getStat().removeExpAndSp((player.getExp() - ExperienceData.getInstance().getExpForLevel(player.getLevel() - 1)), 0);
					player.broadcastUserInfo();
					return "1002000.htm";
				}
				return "1002000-1.htm";
			}
		}
		
		return null;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "1002000.htm";
	}
	
	public static void main(String[] args)
	{
		new DelevelManager();
	}
}
