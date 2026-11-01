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
package ai.others.BlackMarketeerOfMammon;

import java.util.StringTokenizer;

import org.l2jmobius.commons.util.StringUtil;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Black Marketeer of Mammon AI.
 * @author St3eT
 */
public class BlackMarketeerOfMammon extends Script
{
	// NPC
	private static final int BLACK_MARKETEER = 31092;
	
	private BlackMarketeerOfMammon()
	{
		addStartNpc(BLACK_MARKETEER);
		addTalkId(BLACK_MARKETEER);
		addFirstTalkId(BLACK_MARKETEER);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		if (event.equals("31092-01.html"))
		{
			return event;
		}
		else if (event.startsWith("exchange"))
		{
			final StringTokenizer st = new StringTokenizer(event, " ");
			htmltext = st.nextToken();
			if (!st.hasMoreElements())
			{
				return "31092-02.html";
			}
			
			final String value = st.nextToken();
			if (!StringUtil.isNumeric(value))
			{
				return "31092-02.html";
			}
			
			final long count = Integer.parseInt(value);
			final long AAcount = player.getAncientAdena();
			if (count < 1)
			{
				return "31092-02.html";
			}
			
			if (count > AAcount)
			{
				return "31092-03.html";
			}
			
			takeItems(player, Inventory.ANCIENT_ADENA_ID, count);
			giveAdena(player, count, false);
			return "31092-04.html";
		}
		
		return super.onEvent(htmltext, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	public static void main(String[] args)
	{
		new BlackMarketeerOfMammon();
	}
}
