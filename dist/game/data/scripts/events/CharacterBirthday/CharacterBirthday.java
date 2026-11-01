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
package events.CharacterBirthday;

import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.util.Calendar;
import java.util.GregorianCalendar;

import org.l2jmobius.commons.database.DatabaseFactory;
import org.l2jmobius.commons.time.TimeUtil;
import org.l2jmobius.gameserver.config.GeneralConfig;
import org.l2jmobius.gameserver.data.sql.CharInfoTable;
import org.l2jmobius.gameserver.managers.MailManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.OnDailyReset;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.itemcontainer.Mail;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.enums.MailType;
import org.l2jmobius.gameserver.network.holders.MailMessage;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * @author Nyaran, Mobius
 */
public class CharacterBirthday extends Script
{
	// NPCs
	private static final int ALEGRIA = 32600;
	private static final int[] GATEKEEPERS =
	{
		30006,
		30059,
		30080,
		30134,
		30146,
		30177,
		30233,
		30256,
		30320,
		30540,
		30576,
		30836,
		30848,
		30878,
		30899,
		31275,
		31320,
		31964,
		32163
	};
	
	// Query
	private static final String SELECT_PENDING_BIRTHDAY_GIFTS = "SELECT charId, createDate FROM characters WHERE createDate LIKE ?";
	
	// Misc
	private static int _spawnCount = 0;
	private static int _birthdayGiftCount = 0;
	
	private CharacterBirthday()
	{
		addStartNpc(ALEGRIA);
		addStartNpc(GATEKEEPERS);
		addTalkId(ALEGRIA);
		addTalkId(GATEKEEPERS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		if (event.equalsIgnoreCase("despawn_npc"))
		{
			npc.doDie(player);
			_spawnCount--;
			
			htmltext = null;
		}
		else if (event.equalsIgnoreCase("change"))
		{
			// Change Hat
			if (hasQuestItems(player, 10250))
			{
				takeItems(player, 10250, 1); // Adventurer Hat (Event)
				giveItems(player, 21594, 1); // Birthday Hat
				htmltext = null; // FIXME: Probably has html
				
				// Despawn npc
				npc.doDie(player);
				_spawnCount--;
			}
			else
			{
				htmltext = "32600-nohat.htm";
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		if (_spawnCount >= 3)
		{
			return "busy.htm";
		}
		
		if (!LocationUtil.checkIfInRange(10, npc, player, true))
		{
			final Npc spawned = addSpawn(32600, player.getX() + 10, player.getY() + 10, player.getZ() + 20, 0, false, 0, true);
			startQuestTimer("despawn_npc", 180000, spawned, player);
			_spawnCount++;
		}
		else
		{
			return "tooclose.htm";
		}
		
		return null;
	}
	
	@RegisterEvent(EventType.ON_DAILY_RESET)
	@RegisterType(ListenerRegisterType.GLOBAL)
	public void onDailyReset(OnDailyReset event)
	{
		final Calendar lastExecDate = Calendar.getInstance();
		lastExecDate.setTimeInMillis(System.currentTimeMillis() - (24 * 60 * 60 * 1000)); // Last 24 hours.
		
		final Calendar today = Calendar.getInstance();
		final String rangeDate = "[" + TimeUtil.getDateString(lastExecDate.getTime()) + "] - [" + TimeUtil.getDateString(today.getTime()) + "]";
		for (; !today.before(lastExecDate); lastExecDate.add(Calendar.DATE, 1))
		{
			checkBirthday(lastExecDate.get(Calendar.YEAR), lastExecDate.get(Calendar.MONTH), lastExecDate.get(Calendar.DATE));
		}
		
		LOGGER.info(getClass().getSimpleName() + " " + _birthdayGiftCount + " gifts sent. " + rangeDate);
	}
	
	private static void checkBirthday(int year, int month, int day)
	{
		_birthdayGiftCount = 0;
		try (Connection con = DatabaseFactory.getConnection();
			PreparedStatement statement = con.prepareStatement(SELECT_PENDING_BIRTHDAY_GIFTS))
		{
			statement.setString(1, "%-" + getNum(month + 1) + "-" + getNum(day));
			try (ResultSet rset = statement.executeQuery())
			{
				while (rset.next())
				{
					final int playerId = rset.getInt("charId");
					final Calendar createDate = Calendar.getInstance();
					createDate.setTime(rset.getDate("createDate"));
					
					final int age = year - createDate.get(Calendar.YEAR);
					if (age <= 0)
					{
						continue;
					}
					
					String text = GeneralConfig.ALT_BIRTHDAY_MAIL_TEXT;
					if (text.contains("$c1"))
					{
						text = text.replace("$c1", CharInfoTable.getInstance().getNameById(playerId));
					}
					
					if (text.contains("$s1"))
					{
						text = text.replace("$s1", String.valueOf(age));
					}
					
					final MailMessage message = new MailMessage(playerId, GeneralConfig.ALT_BIRTHDAY_MAIL_SUBJECT, text, MailType.BIRTHDAY);
					final Mail attachments = message.createAttachments();
					attachments.addItem(ItemProcessType.REWARD, GeneralConfig.ALT_BIRTHDAY_GIFT, 1, null, null);
					MailManager.getInstance().sendMessage(message);
					_birthdayGiftCount++;
				}
			}
		}
		catch (Exception e)
		{
			LOGGER.warning(CharacterBirthday.class.getSimpleName() + ": Error checking birthdays. " + e.getMessage());
		}
		
		// If character birthday is 29-Feb and year is not leap, send gift on 28-feb.
		final GregorianCalendar calendar = new GregorianCalendar();
		if ((month == Calendar.FEBRUARY) && (day == 28) && !calendar.isLeapYear(Calendar.getInstance().get(Calendar.YEAR)))
		{
			checkBirthday(year, Calendar.FEBRUARY, 29);
		}
	}
	
	/**
	 * @param num the number to format.
	 * @return the formatted number starting with a 0 if it is lower or equal than 10.
	 */
	private static String getNum(int num)
	{
		return (num <= 9) ? "0" + num : String.valueOf(num);
	}
	
	public static void main(String[] args)
	{
		new CharacterBirthday();
	}
}
