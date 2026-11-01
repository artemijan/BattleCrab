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
package events.DandysDragonWeapons;

import java.sql.Connection;
import java.sql.PreparedStatement;

import org.l2jmobius.commons.database.DatabaseFactory;
import org.l2jmobius.gameserver.data.xml.MultisellData;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.OnDailyReset;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.script.LongTimeEvent;

/**
 * Dandy's Dragon Weapons.
 * @URL http://www.lineage2.com/en/news/events/dandys-dragon-weapons.php
 * @author ChaosPaladin
 */
public final class DandysDragonWeapons extends LongTimeEvent
{
	// NPCs
	private static final int DANDI = 33930;
	
	// Items
	private static final int DANDYS_SCROLL_RELEASE_SEAL = 38847;
	private static final int DRAGON_SCROLL_BOOST_WEAPON = 38848;
	
	// Variables
	private static final String GIVE_DANDI_SCROLL = "GIVE_DANDI_SCROLL";
	private static final String GIVE_DANDI_SCROLL_PCCAFE = "GIVE_DANDI_SCROLL_PCCAFE";
	
	private DandysDragonWeapons()
	{
		addStartNpc(DANDI);
		addFirstTalkId(DANDI);
		addTalkId(DANDI);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "ev_10th_dandi001.htm":
			case "ev_10th_dandi002.htm":
			case "ev_10th_dandi003.htm":
			case "ev_10th_dandi004.htm":
			case "ev_10th_dandi005.htm":
			case "ev_10th_dandi006.htm":
			case "ev_10th_dandi007.htm":
			case "ev_10th_dandi008.htm":
			case "ev_10th_dandi009.htm":
			{
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "ev_10th_dandi001.htm";
	}
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(DANDI)
	public void OnNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == -10001000)
		{
			if (reply == 1)
			{
				MultisellData.getInstance().separateAndSend(2500, player, npc, false); // Custom ID
			}
			else if (reply == 2)
			{
				MultisellData.getInstance().separateAndSend(3393001, player, npc, false); // Custom ID
			}
		}
		else if (ask == -10001001)
		{
			if (reply == 1)
			{
				if (!player.getVariables().getBoolean(GIVE_DANDI_SCROLL, false))
				{
					final int i = getRandom(10);
					if (i < 9)
					{
						giveItems(player, DANDYS_SCROLL_RELEASE_SEAL, 1);
						showHtmlFile(player, "ev_10th_dandi004.htm");
					}
					else
					{
						giveItems(player, DRAGON_SCROLL_BOOST_WEAPON, 1);
						showHtmlFile(player, "ev_10th_dandi005.htm");
					}
					
					player.getVariables().set(GIVE_DANDI_SCROLL, true);
				}
				else
				{
					showHtmlFile(player, "ev_10th_dandi006.htm");
				}
			}
			else if (reply == 2)
			{
				if (player.getPcCafePoints() < 120)
				{
					showHtmlFile(player, "ev_10th_dandi009.htm");
				}
				else if (!player.getVariables().getBoolean(GIVE_DANDI_SCROLL_PCCAFE, false))
				{
					player.getVariables().set(GIVE_DANDI_SCROLL_PCCAFE, true);
					player.setPcCafePoints(player.getPcCafePoints() - 120);
					giveItems(player, DANDYS_SCROLL_RELEASE_SEAL, 1);
					showHtmlFile(player, "ev_10th_dandi007.htm");
				}
				else
				{
					showHtmlFile(player, "ev_10th_dandi008.htm");
				}
			}
		}
	}
	
	@RegisterEvent(EventType.ON_DAILY_RESET)
	@RegisterType(ListenerRegisterType.GLOBAL)
	public void onDailyReset(OnDailyReset event)
	{
		if (!isEventPeriod())
		{
			return;
		}
		
		// Update data for offline players.
		try (Connection con = DatabaseFactory.getConnection())
		{
			try (PreparedStatement ps = con.prepareStatement("DELETE FROM character_variables WHERE var IN (?, ?)"))
			{
				ps.setString(1, GIVE_DANDI_SCROLL);
				ps.setString(2, GIVE_DANDI_SCROLL_PCCAFE);
				ps.execute();
			}
		}
		catch (Exception e)
		{
			LOGGER.warning(getClass().getSimpleName() + ": Could not reset variables: " + e.getMessage());
		}
		
		// Update data for online players.
		for (Player player : World.getInstance().getPlayers())
		{
			player.getVariables().remove(GIVE_DANDI_SCROLL);
			player.getVariables().remove(GIVE_DANDI_SCROLL_PCCAFE);
		}
		
		LOGGER.info(getClass().getSimpleName() + " has been reset.");
	}
	
	public static void main(String[] args)
	{
		new DandysDragonWeapons();
	}
}
