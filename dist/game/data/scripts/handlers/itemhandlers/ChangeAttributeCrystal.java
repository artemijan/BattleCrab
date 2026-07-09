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
package handlers.itemhandlers;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.l2jmobius.gameserver.enums.ItemGrade;
import org.l2jmobius.gameserver.enums.PrivateStoreType;
import org.l2jmobius.gameserver.handler.IItemHandler;
import org.l2jmobius.gameserver.model.ItemInfo;
import org.l2jmobius.gameserver.model.actor.Playable;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.attributechange.ExChangeAttributeItemList;

/**
 * @author Mobius
 */
public class ChangeAttributeCrystal implements IItemHandler
{
	private static final Map<Integer, ItemGrade> ITEM_GRADES = new HashMap<>();
	static
	{
		ITEM_GRADES.put(33502, ItemGrade.S);
		ITEM_GRADES.put(35749, ItemGrade.R);
		ITEM_GRADES.put(45817, ItemGrade.R);
	}
	
	@Override
	public boolean useItem(Playable playable, Item item, boolean forceUse)
	{
		if (!playable.isPlayer())
		{
			playable.sendPacket(SystemMessageId.YOUR_PET_CANNOT_CARRY_THIS_ITEM);
			return false;
		}
		
		final Player player = playable.getActingPlayer();
		if (player.getPrivateStoreType() != PrivateStoreType.NONE)
		{
			player.sendPacket(SystemMessageId.YOU_CANNOT_CHANGE_AN_ATTRIBUTE_WHILE_USING_A_PRIVATE_STORE_OR_WORKSHOP);
			return false;
		}
		
		if (ITEM_GRADES.get(item.getId()) == null)
		{
			player.sendPacket(SystemMessageId.CHANGING_ATTRIBUTES_HAS_BEEN_FAILED);
			return false;
		}
		
		final List<ItemInfo> itemList = new ArrayList<>();
		for (Item i : player.getInventory().getItems())
		{
			if (i.isWeapon() && i.hasAttributes() && (i.getTemplate().getItemGrade() == ITEM_GRADES.get(item.getId())))
			{
				itemList.add(new ItemInfo(i));
			}
		}
		
		if (itemList.isEmpty())
		{
			player.sendPacket(SystemMessageId.THE_ITEM_FOR_CHANGING_AN_ATTRIBUTE_DOES_NOT_EXIST);
			return false;
		}
		
		player.sendPacket(new ExChangeAttributeItemList(item.getId(), itemList));
		return true;
	}
}