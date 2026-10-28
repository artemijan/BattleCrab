/*
 * Copyright (c) 2013 L2jMobius
 * 
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 * 
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 * 
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
 * WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
 * IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
package ai.others.ProvisionalHalls;

import java.util.Calendar;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Map.Entry;
import java.util.concurrent.ConcurrentHashMap;

import org.l2jmobius.gameserver.data.xml.ClanHallData;
import org.l2jmobius.gameserver.data.xml.ItemData;
import org.l2jmobius.gameserver.managers.GlobalVariablesManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.clan.Clan;
import org.l2jmobius.gameserver.model.residences.ClanHall;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Custom implementation for Provisional Clan Halls.
 * @author Mobius
 */
public class ProvisionalHalls extends Script
{
	// NPCs
	private static final int KERRY = 33359;
	private static final int MAID = 33360;
	
	// Misc
	private static final int CURRENCY = 57;
	private static final int HALL_PRICE = 50000000;
	private static final long TWO_WEEKS = 1209600000;
	private static final Map<Integer, Location> CLAN_HALLS = new LinkedHashMap<>();
	static
	{
		CLAN_HALLS.put(187, new Location(-122200, -116552, -5798, 1779));
		CLAN_HALLS.put(186, new Location(-122264, -122392, -5870, 15229));
		CLAN_HALLS.put(188, new Location(-121864, -111240, -6014, 30268));
		CLAN_HALLS.put(190, new Location(-117080, -116551, -5771, 1779));
		CLAN_HALLS.put(189, new Location(-117000, -122052, -5845, 15229));
		CLAN_HALLS.put(191, new Location(-117074, -111237, -5989, 30268));
		CLAN_HALLS.put(193, new Location(-111717, -116550, -5773, 1779));
		CLAN_HALLS.put(192, new Location(-111726, -122378, -5845, 15229));
		CLAN_HALLS.put(194, new Location(-111158, -111230, -5989, 30268));
	}
	private static final String HALL_OWNER_VAR = "PCH_OWNER_";
	private static final String HALL_TIME_VAR = "PCH_TIME_";
	private static final String HALL_RESET_VAR = "PCH_RESET_";
	private static final String HALL_RETURN_VAR = "PCH_RETURN";
	private static final Map<Integer, Object> HALL_LOCKS = new ConcurrentHashMap<>();
	
	private ProvisionalHalls()
	{
		addStartNpc(KERRY);
		addFirstTalkId(KERRY);
		addTalkId(KERRY);
		
		for (int id : CLAN_HALLS.keySet())
		{
			final long resetTime = GlobalVariablesManager.getInstance().getLong(HALL_TIME_VAR + id, 0);
			if (resetTime > 0)
			{
				cancelQuestTimers(HALL_RESET_VAR + id);
				startQuestTimer(HALL_RESET_VAR + id, Math.max(1000, (TWO_WEEKS - (System.currentTimeMillis() - resetTime) - 30000)), null, null);
			}
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		if (event.equals("33359-01.html") || event.equals("33359-02.html") || event.equals("33359-03.html"))
		{
			htmltext = event;
		}
		else if (event.equals("buy"))
		{
			if ((npc == null) || (npc.getId() != KERRY))
			{
				return null;
			}
			
			final Calendar calendar = Calendar.getInstance();
			final int dayOfWeek = calendar.get(Calendar.DAY_OF_WEEK);
			final Clan clan = player.getClan();
			if ((clan == null) || (clan.getLeaderId() != player.getObjectId()))
			{
				player.sendMessage("You need to be a clan leader in order to proceed.");
			}
			else if ((clan.getHideoutId() > 0))
			{
				player.sendMessage("You already own a hideout.");
			}
			else if ((dayOfWeek != Calendar.SATURDAY) && (dayOfWeek != Calendar.SUNDAY))
			{
				htmltext = "33359-02.html";
			}
			else if (getQuestItemsCount(player, CURRENCY) < HALL_PRICE)
			{
				player.sendMessage("You need " + HALL_PRICE + " " + ItemData.getInstance().getTemplate(CURRENCY).getName() + " in order to proceed.");
			}
			else
			{
				if (dayOfWeek != Calendar.SATURDAY)
				{
					calendar.add(Calendar.DAY_OF_WEEK, -1);
				}
				
				calendar.set(Calendar.HOUR_OF_DAY, 0);
				calendar.set(Calendar.MINUTE, 1);
				calendar.set(Calendar.SECOND, 0);
				calendar.set(Calendar.MILLISECOND, 0);
				
				for (int id : CLAN_HALLS.keySet())
				{
					final Object hallLock = HALL_LOCKS.computeIfAbsent(id, k -> new Object());
					synchronized (hallLock)
					{
						if ((GlobalVariablesManager.getInstance().getInt(HALL_OWNER_VAR + id, 0) == 0) && ((GlobalVariablesManager.getInstance().getLong(HALL_TIME_VAR + id, 0) + TWO_WEEKS) < System.currentTimeMillis()))
						{
							takeItems(player, CURRENCY, HALL_PRICE);
							GlobalVariablesManager.getInstance().set(HALL_OWNER_VAR + id, player.getClanId());
							GlobalVariablesManager.getInstance().set(HALL_TIME_VAR + id, calendar.getTimeInMillis());
							final ClanHall clanHall = ClanHallData.getInstance().getClanHallById(id);
							if (clanHall != null)
							{
								clanHall.setOwner(clan);
							}
							
							player.sendMessage("Congratulations! You now own a provisional clan hall!");
							startQuestTimer(HALL_RESET_VAR + id, TWO_WEEKS - (System.currentTimeMillis() - calendar.getTimeInMillis()), null, null);
							return null;
						}
					}
				}
				
				player.sendMessage("I am sorry, all halls have been taken.");
			}
		}
		else if (event.equals("enter"))
		{
			if ((npc == null) || (npc.getId() != KERRY))
			{
				return null;
			}
			
			final int playerClanId = player.getClanId();
			for (Entry<Integer, Location> hall : CLAN_HALLS.entrySet())
			{
				if (playerClanId == GlobalVariablesManager.getInstance().getInt(HALL_OWNER_VAR + hall.getKey(), -1))
				{
					player.getVariables().set(HALL_RETURN_VAR, player.getX() + "," + player.getY() + "," + player.getZ() + "," + player.getHeading());
					player.teleToLocation(hall.getValue());
					return null;
				}
			}
			
			htmltext = "33359-02.html";
		}
		else if (event.equals("leave"))
		{
			if ((npc == null) || (npc.getId() != MAID))
			{
				return null;
			}
			
			final String[] location = player.getVariables().getString(HALL_RETURN_VAR, "-112574,256057,-1449,-1").split(",");
			player.teleToLocation(Integer.parseInt(location[0]), Integer.parseInt(location[1]), Integer.parseInt(location[2]), Integer.parseInt(location[3]));
		}
		else if (event.startsWith(HALL_RESET_VAR))
		{
			final String id = event.replace(HALL_RESET_VAR, "");
			if (((GlobalVariablesManager.getInstance().getLong(HALL_TIME_VAR + id, 0) + TWO_WEEKS) - 60000) <= System.currentTimeMillis())
			{
				final int clanId = GlobalVariablesManager.getInstance().getInt(HALL_OWNER_VAR + id, 0);
				if (clanId > 0)
				{
					final ClanHall clanHall = ClanHallData.getInstance().getClanHallById(Integer.parseInt(id));
					if (clanHall != null)
					{
						clanHall.setOwner(null);
					}
				}
				
				GlobalVariablesManager.getInstance().remove(HALL_TIME_VAR + id);
				GlobalVariablesManager.getInstance().remove(HALL_OWNER_VAR + id);
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final Calendar calendar = Calendar.getInstance();
		final int dayOfWeek = calendar.get(Calendar.DAY_OF_WEEK);
		if ((dayOfWeek != Calendar.SATURDAY) && (dayOfWeek != Calendar.SUNDAY))
		{
			return "33359-01.html";
		}
		
		return "33359-01b.html";
	}
	
	public static void main(String[] args)
	{
		new ProvisionalHalls();
	}
}
