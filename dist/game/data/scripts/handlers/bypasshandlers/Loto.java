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
package handlers.bypasshandlers;

import java.text.DateFormat;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.handler.IBypassHandler;
import org.l2jmobius.gameserver.instancemanager.IdManager;
import org.l2jmobius.gameserver.instancemanager.games.Lottery;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ActionFailed;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

public class Loto implements IBypassHandler
{
	private static final String[] COMMANDS =
	{
		"Loto"
	};
	
	@Override
	public boolean useBypass(String command, Player player, Creature target)
	{
		if (!target.isNpc())
		{
			return false;
		}
		
		int val = 0;
		try
		{
			val = Integer.parseInt(command.substring(5));
		}
		catch (Exception e)
		{
			// Handled above.
		}
		
		if (val == 0)
		{
			// new loto ticket
			for (int i = 0; i < 5; i++)
			{
				player.setLoto(i, 0);
			}
		}
		showLotoWindow(player, (Npc) target, val);
		return false;
	}
	
	/**
	 * Open a Loto window on client with the text of the Npc.<br>
	 * <br>
	 * <b><u>Actions</u>:</b><br>
	 * <li>Get the text of the selected HTML file in function of the npcId and of the page number</li>
	 * <li>Send a Server->Client NpcHtmlMessage containing the text of the Npc to the Player</li>
	 * <li>Send a Server->Client ActionFailed to the Player in order to avoid that the client wait another packet</li><br>
	 * @param player The Player that talk with the Npc
	 * @param npc Npc loto instance
	 * @param value The number of the page of the Npc to display
	 */
	// 0 - first buy lottery ticket window
	// 1-20 - buttons
	// 21 - second buy lottery ticket window
	// 22 - selected ticket with 5 numbers
	// 23 - current lottery jackpot
	// 24 - Previous winning numbers/Prize claim
	// >24 - check lottery ticket by item object id
	public static void showLotoWindow(Player player, Npc npc, int value)
	{
		final int npcId = npc.getTemplate().getId();
		String filename;
		SystemMessage sm;
		final NpcHtmlMessage html = new NpcHtmlMessage(npc.getObjectId());
		if (value == 0) // 0 - first buy lottery ticket window
		{
			filename = (npc.getHtmlPath(npcId, 1, player));
			html.setFile(player, filename);
		}
		else if ((value >= 1) && (value <= 21)) // 1-20 - buttons, 21 - second buy lottery ticket window
		{
			if (!Lottery.getInstance().isStarted())
			{
				// tickets can't be sold
				player.sendPacket(SystemMessageId.LOTTERY_TICKETS_ARE_NOT_CURRENTLY_BEING_SOLD);
				return;
			}
			if (!Lottery.getInstance().isSellableTickets())
			{
				// tickets can't be sold
				player.sendPacket(SystemMessageId.TICKETS_FOR_THE_CURRENT_LOTTERY_ARE_NO_LONGER_AVAILABLE);
				return;
			}
			
			filename = (npc.getHtmlPath(npcId, 5, player));
			html.setFile(player, filename);
			int count = 0;
			int found = 0;
			// counting buttons and unsetting button if found
			for (int i = 0; i < 5; i++)
			{
				if (player.getLoto(i) == value)
				{
					// unsetting button
					player.setLoto(i, 0);
					found = 1;
				}
				else if (player.getLoto(i) > 0)
				{
					count++;
				}
			}
			
			// if not rearched limit 5 and not unseted value
			if ((count < 5) && (found == 0) && (value <= 20))
			{
				for (int i = 0; i < 5; i++)
				{
					if (player.getLoto(i) == 0)
					{
						player.setLoto(i, value);
						break;
					}
				}
			}
			
			// setting pusshed buttons
			count = 0;
			for (int i = 0; i < 5; i++)
			{
				if (player.getLoto(i) > 0)
				{
					count++;
					String button = String.valueOf(player.getLoto(i));
					if (player.getLoto(i) < 10)
					{
						button = "0" + button;
					}
					final String search = "fore=\"L2UI.lottoNum" + button + "\" back=\"L2UI.lottoNum" + button + "a_check\"";
					final String replace = "fore=\"L2UI.lottoNum" + button + "a_check\" back=\"L2UI.lottoNum" + button + "\"";
					html.replace(search, replace);
				}
			}
			
			if (count == 5)
			{
				final String search = "0\">Return";
				final String replace = "22\">Your lucky numbers have been selected above.";
				html.replace(search, replace);
			}
		}
		else if (value == 22) // 22 - selected ticket with 5 numbers
		{
			if (!Lottery.getInstance().isStarted())
			{
				// tickets can't be sold
				player.sendPacket(SystemMessageId.LOTTERY_TICKETS_ARE_NOT_CURRENTLY_BEING_SOLD);
				return;
			}
			if (!Lottery.getInstance().isSellableTickets())
			{
				// tickets can't be sold
				player.sendPacket(SystemMessageId.TICKETS_FOR_THE_CURRENT_LOTTERY_ARE_NO_LONGER_AVAILABLE);
				return;
			}
			
			final long price = Config.ALT_LOTTERY_TICKET_PRICE;
			final int lotonumber = Lottery.getInstance().getId();
			int enchant = 0;
			int type2 = 0;
			for (int i = 0; i < 5; i++)
			{
				if (player.getLoto(i) == 0)
				{
					return;
				}
				
				if (player.getLoto(i) < 17)
				{
					enchant += Math.pow(2, player.getLoto(i) - 1);
				}
				else
				{
					type2 += Math.pow(2, player.getLoto(i) - 17);
				}
			}
			if (player.getAdena() < price)
			{
				sm = new SystemMessage(SystemMessageId.YOU_DO_NOT_HAVE_ENOUGH_ADENA);
				player.sendPacket(sm);
				return;
			}
			if (!player.reduceAdena("Loto", price, npc, true))
			{
				return;
			}
			Lottery.getInstance().increasePrize(price);
			
			sm = new SystemMessage(SystemMessageId.YOU_HAVE_EARNED_S1);
			sm.addItemName(4442);
			player.sendPacket(sm);
			
			final Item item = new Item(IdManager.getInstance().getNextId(), 4442);
			item.setCount(1);
			item.setCustomType1(lotonumber);
			item.setEnchantLevel(enchant);
			item.setCustomType2(type2);
			player.getInventory().addItem("Loto", item, player, npc);
			filename = (npc.getHtmlPath(npcId, 6, player));
			html.setFile(player, filename);
		}
		else if (value == 23) // 23 - current lottery jackpot
		{
			filename = (npc.getHtmlPath(npcId, 3, player));
			html.setFile(player, filename);
		}
		else if (value == 24) // 24 - Previous winning numbers/Prize claim
		{
			filename = (npc.getHtmlPath(npcId, 4, player));
			html.setFile(player, filename);
			
			final int lotonumber = Lottery.getInstance().getId();
			String message = "";
			for (Item item : player.getInventory().getItems())
			{
				if (item == null)
				{
					continue;
				}
				if ((item.getId() == 4442) && (item.getCustomType1() < lotonumber))
				{
					message = message + "<a action=\"bypass -h npc_%objectId%_Loto " + item.getObjectId() + "\">" + item.getCustomType1() + " Event Number ";
					final int[] numbers = Lottery.getInstance().decodeNumbers(item.getEnchantLevel(), item.getCustomType2());
					for (int i = 0; i < 5; i++)
					{
						message += numbers[i] + " ";
					}
					final long[] check = Lottery.getInstance().checkTicket(item);
					if (check[0] > 0)
					{
						switch ((int) check[0])
						{
							case 1:
							{
								message += "- 1st Prize";
								break;
							}
							case 2:
							{
								message += "- 2nd Prize";
								break;
							}
							case 3:
							{
								message += "- 3th Prize";
								break;
							}
							case 4:
							{
								message += "- 4th Prize";
								break;
							}
						}
						message += " " + check[1] + "a.";
					}
					message += "</a><br>";
				}
			}
			if (message.isEmpty())
			{
				message += "There has been no winning lottery ticket.<br>";
			}
			html.replace("%result%", message);
		}
		else if (value == 25) // 25 - lottery instructions
		{
			filename = (npc.getHtmlPath(npcId, 2, player));
			html.setFile(player, filename);
		}
		else if (value > 25) // >25 - check lottery ticket by item object id
		{
			final int lotonumber = Lottery.getInstance().getId();
			final Item item = player.getInventory().getItemByObjectId(value);
			if ((item == null) || (item.getId() != 4442) || (item.getCustomType1() >= lotonumber))
			{
				return;
			}
			final long[] check = Lottery.getInstance().checkTicket(item);
			sm = new SystemMessage(SystemMessageId.S1_HAS_DISAPPEARED);
			sm.addItemName(4442);
			player.sendPacket(sm);
			
			final long adena = check[1];
			if (adena > 0)
			{
				player.addAdena("Loto", adena, npc, true);
			}
			player.destroyItem("Loto", item, npc, false);
			return;
		}
		html.replace("%objectId%", String.valueOf(npc.getObjectId()));
		html.replace("%race%", Integer.toString(Lottery.getInstance().getId()));
		html.replace("%adena%", Long.toString(Lottery.getInstance().getPrize()));
		html.replace("%ticket_price%", Long.toString(Config.ALT_LOTTERY_TICKET_PRICE));
		html.replace("%prize5%", Float.toString(Config.ALT_LOTTERY_5_NUMBER_RATE * 100));
		html.replace("%prize4%", Float.toString(Config.ALT_LOTTERY_4_NUMBER_RATE * 100));
		html.replace("%prize3%", Float.toString(Config.ALT_LOTTERY_3_NUMBER_RATE * 100));
		html.replace("%prize2%", Long.toString(Config.ALT_LOTTERY_2_AND_1_NUMBER_PRIZE));
		html.replace("%enddate%", DateFormat.getDateInstance().format(Lottery.getInstance().getEndDate()));
		player.sendPacket(html);
		
		// Send a Server->Client ActionFailed to the Player in order to avoid that the client wait another packet
		player.sendPacket(ActionFailed.STATIC_PACKET);
	}
	
	@Override
	public String[] getBypassList()
	{
		return COMMANDS;
	}
}
