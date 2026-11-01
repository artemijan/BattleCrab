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
package ai.areas.TownOfGiran.Alexandria;

import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemChanceHolder;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Alexandria (Armor Merchant) AI.
 * @author xban1x
 */
public class Alexandria extends Script
{
	// NPC
	private static final int ALEXANDRIA = 30098;
	
	// Items
	private static final ItemHolder[] REQUIRED_ITEMS = new ItemHolder[]
	{
		new ItemHolder(57, 3550000),
		new ItemHolder(5094, 400),
		new ItemHolder(6471, 200),
		new ItemHolder(9814, 40),
		new ItemHolder(9815, 30),
		new ItemHolder(9816, 50),
		new ItemHolder(9817, 50),
	};
	
	// Agathions
	private static final ItemChanceHolder[] LITTLE_DEVILS = new ItemChanceHolder[]
	{
		new AdditionalItemChanceHolder(10321, 600, 1, 10408),
		new ItemChanceHolder(10322, 10),
		new ItemChanceHolder(10323, 10),
		new ItemChanceHolder(10324, 5),
		new ItemChanceHolder(10325, 5),
		new ItemChanceHolder(10326, 370),
	};
	private static final ItemChanceHolder[] LITTLE_ANGELS = new ItemChanceHolder[]
	{
		new AdditionalItemChanceHolder(10315, 600, 1, 10408),
		new ItemChanceHolder(10316, 10),
		new ItemChanceHolder(10317, 10),
		new ItemChanceHolder(10318, 5),
		new ItemChanceHolder(10319, 5),
		new ItemChanceHolder(10320, 370),
	};
	private static final Map<String, List<ItemChanceHolder>> AGATHIONS = new HashMap<>();
	static
	{
		AGATHIONS.put("littleAngel", Arrays.asList(LITTLE_ANGELS));
		AGATHIONS.put("littleDevil", Arrays.asList(LITTLE_DEVILS));
	}
	
	private Alexandria()
	{
		addStartNpc(ALEXANDRIA);
		addTalkId(ALEXANDRIA);
		addFirstTalkId(ALEXANDRIA);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		if (event.equals("30098-02.html"))
		{
			htmltext = event;
		}
		else if (AGATHIONS.containsKey(event))
		{
			final int chance = getRandom(1000);
			int chance2 = 0;
			int chance3 = 0;
			for (ItemChanceHolder agathion : AGATHIONS.get(event))
			{
				chance3 += agathion.getChance();
				if ((chance >= chance2) && (chance2 < chance3))
				{
					if (takeAllItems(player, REQUIRED_ITEMS))
					{
						giveItems(player, agathion);
						htmltext = "30098-03.html";
						if (agathion instanceof AdditionalItemChanceHolder)
						{
							giveItems(player, ((AdditionalItemChanceHolder) agathion).getAdditionalId(), 1);
							htmltext = "30098-03a.html";
						}
					}
					else
					{
						htmltext = "30098-04.html";
					}
					break;
				}
				
				chance2 += agathion.getChance();
			}
		}
		
		return htmltext;
	}
	
	private static class AdditionalItemChanceHolder extends ItemChanceHolder
	{
		private final int _additionalId;
		
		public AdditionalItemChanceHolder(int id, int chance, long count, int additionalId)
		{
			super(id, chance, count);
			_additionalId = additionalId;
		}
		
		public int getAdditionalId()
		{
			return _additionalId;
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	public static void main(String[] args)
	{
		new Alexandria();
	}
}
