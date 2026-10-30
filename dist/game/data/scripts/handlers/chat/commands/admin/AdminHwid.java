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

import org.l2jmobius.gameserver.cache.HtmCache;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.network.GameClient;
import org.l2jmobius.gameserver.network.holders.ClientHardwareInfoHolder;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

/**
 * @author Mobius
 */
public class AdminHwid implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_hwid",
		"admin_hwinfo"
	};
	
	@Override
	public boolean onCommand(String command, Player activeChar)
	{
		if ((activeChar.getTarget() == null) || !activeChar.getTarget().isPlayer())
		{
			return true;
		}
		
		final Player targetPlayer = activeChar.getTarget().asPlayer();
		final GameClient client = targetPlayer.getClient();
		if (client == null)
		{
			return true;
		}
		
		final ClientHardwareInfoHolder info = client.getHardwareInfo();
		if (info == null)
		{
			return true;
		}
		
		final NpcHtmlMessage html = new NpcHtmlMessage(0, 1);
		html.setHtml(HtmCache.getInstance().getHtm(activeChar, "data/html/admin/charhwinfo.htm"));
		html.replace("%name%", targetPlayer.getName());
		html.replace("%macAddress%", info.getMacAddress());
		html.replace("%windowsPlatformId%", info.getWindowsPlatformId());
		html.replace("%windowsMajorVersion%", info.getWindowsMajorVersion());
		html.replace("%windowsMinorVersion%", info.getWindowsMinorVersion());
		html.replace("%windowsBuildNumber%", info.getWindowsBuildNumber());
		html.replace("%cpuName%", info.getCpuName());
		html.replace("%cpuSpeed%", info.getCpuSpeed());
		html.replace("%cpuCoreCount%", info.getCpuCoreCount());
		html.replace("%vgaName%", info.getVgaName());
		html.replace("%vgaDriverVersion%", info.getVgaDriverVersion());
		activeChar.sendPacket(html);
		return true;
	}
	
	@Override
	public String[] getCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
