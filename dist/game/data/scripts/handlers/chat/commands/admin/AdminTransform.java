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

import org.l2jmobius.commons.util.StringUtil;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.network.SystemMessageId;

/**
 * @author Mobius
 */
public class AdminTransform implements IAdminCommandHandler
{
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_transform",
		"admin_untransform",
		"admin_transform_menu",
	};
	
	@Override
	public boolean onCommand(String command, Player activeChar)
	{
		if (command.equals("admin_transform_menu"))
		{
			AdminHtml.showAdminHtml(activeChar, "transform.htm");
			return true;
		}
		else if (command.startsWith("admin_untransform"))
		{
			final WorldObject obj = activeChar.getTarget() == null ? activeChar : activeChar.getTarget();
			if (!obj.isCreature() || !obj.asCreature().isTransformed())
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				return false;
			}
			
			obj.asCreature().stopTransformation(true);
		}
		else if (command.startsWith("admin_transform"))
		{
			final WorldObject obj = activeChar.getTarget();
			if ((obj == null) || !obj.isPlayer())
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				return false;
			}
			
			final Player player = obj.asPlayer();
			if (activeChar.isSitting())
			{
				activeChar.sendPacket(SystemMessageId.YOU_CANNOT_TRANSFORM_WHILE_SITTING);
				return false;
			}
			
			if (player.isTransformed())
			{
				if (!command.contains(" "))
				{
					player.untransform();
					return true;
				}
				
				activeChar.sendPacket(SystemMessageId.YOU_ALREADY_POLYMORPHED_AND_CANNOT_POLYMORPH_AGAIN);
				return false;
			}
			
			if (player.isInWater())
			{
				activeChar.sendPacket(SystemMessageId.YOU_CANNOT_POLYMORPH_INTO_THE_DESIRED_FORM_IN_WATER);
				return false;
			}
			
			if (player.isFlyingMounted() || player.isMounted())
			{
				activeChar.sendPacket(SystemMessageId.YOU_CANNOT_TRANSFORM_WHILE_RIDING_A_PET);
				return false;
			}
			
			final String[] parts = command.split(" ");
			if ((parts.length != 2) || !StringUtil.isNumeric(parts[1]))
			{
				activeChar.sendSysMessage("Usage: //transform <id>");
				return false;
			}
			
			final int id = Integer.parseInt(parts[1]);
			if (!player.transform(id, true))
			{
				player.sendMessage("Unknown transformation ID: " + id);
				return false;
			}
		}
		
		return true;
	}
	
	@Override
	public String[] getCommandList()
	{
		return ADMIN_COMMANDS;
	}
}
