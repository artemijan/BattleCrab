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
package handlers.chat.commands.admin;

import java.util.Calendar;
import java.util.StringTokenizer;

import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.managers.GraciaSeedsManager;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

public class AdminGraciaSeeds implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_gracia_seeds",
		"admin_kill_tiat",
		"admin_set_sodstate"
	};
	
	@Override
	public boolean onCommand(String command, Player activeChar)
	{
		final StringTokenizer st = new StringTokenizer(command, " ");
		final String actualCommand = st.nextToken(); // Get actual command
		String val = "";
		if (st.countTokens() >= 1)
		{
			val = st.nextToken();
		}
		
		if (actualCommand.equalsIgnoreCase("admin_kill_tiat"))
		{
			GraciaSeedsManager.getInstance().increaseSoDTiatKilled();
		}
		else if (actualCommand.equalsIgnoreCase("admin_set_sodstate"))
		{
			GraciaSeedsManager.getInstance().setSoDState(Integer.parseInt(val), true);
		}
		
		showMenu(activeChar);
		return true;
	}
	
	private void showMenu(Player activeChar)
	{
		final NpcHtmlMessage html = new NpcHtmlMessage(0, 1);
		html.setFile(activeChar, "data/html/admin/graciaseeds.htm");
		html.replace("%sodstate%", String.valueOf(GraciaSeedsManager.getInstance().getSoDState()));
		html.replace("%sodtiatkill%", String.valueOf(GraciaSeedsManager.getInstance().getSoDTiatKilled()));
		if (GraciaSeedsManager.getInstance().getSoDTimeForNextStateChange() > 0)
		{
			final Calendar nextChangeDate = Calendar.getInstance();
			nextChangeDate.setTimeInMillis(System.currentTimeMillis() + GraciaSeedsManager.getInstance().getSoDTimeForNextStateChange());
			html.replace("%sodtime%", nextChangeDate.getTime().toString());
		}
		else
		{
			html.replace("%sodtime%", "-1");
		}
		
		activeChar.sendPacket(html);
	}
	
	@Override
	public String[] getCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
