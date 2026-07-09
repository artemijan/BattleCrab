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
package handlers.actionhandlers;

import org.l2jmobius.gameserver.ai.CtrlIntention;
import org.l2jmobius.gameserver.cache.HtmCache;
import org.l2jmobius.gameserver.enums.InstanceType;
import org.l2jmobius.gameserver.handler.IActionHandler;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.StaticObject;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

public class StaticObjectAction implements IActionHandler
{
	@Override
	public boolean action(Player player, WorldObject target, boolean interact)
	{
		final StaticObject staticObject = (StaticObject) target;
		if (staticObject.getType() < 0)
		{
			LOGGER.info("StaticObject: StaticObject with invalid type! StaticObjectId: " + staticObject.getId());
		}
		
		// Check if the Player already target the Npc
		if (player.getTarget() != staticObject)
		{
			// Set the target of the Player player
			player.setTarget(staticObject);
		}
		else if (interact)
		{
			// Calculate the distance between the Player and the Npc
			if (!player.isInsideRadius2D(staticObject, Npc.INTERACTION_DISTANCE))
			{
				// Notify the Player AI with AI_INTENTION_INTERACT
				player.getAI().setIntention(CtrlIntention.AI_INTENTION_INTERACT, staticObject);
			}
			else if (staticObject.getType() == 2)
			{
				final String filename = (staticObject.getId() == 24230101) ? "data/html/signboard/tomb_of_crystalgolem.htm" : "data/html/signboard/pvp_signboard.htm";
				final String content = HtmCache.getInstance().getHtm(player, filename);
				final NpcHtmlMessage html = new NpcHtmlMessage(staticObject.getObjectId());
				
				if (content == null)
				{
					html.setHtml("<html><body>Signboard is missing:<br>" + filename + "</body></html>");
				}
				else
				{
					html.setHtml(content);
				}
				
				player.sendPacket(html);
			}
			else if (staticObject.getType() == 0)
			{
				player.sendPacket(staticObject.getMap());
			}
		}
		return true;
	}
	
	@Override
	public InstanceType getInstanceType()
	{
		return InstanceType.StaticObject;
	}
}
