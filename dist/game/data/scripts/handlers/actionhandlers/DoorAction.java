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
import org.l2jmobius.gameserver.data.xml.ClanHallData;
import org.l2jmobius.gameserver.enums.InstanceType;
import org.l2jmobius.gameserver.handler.IActionHandler;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Door;
import org.l2jmobius.gameserver.model.holders.DoorRequestHolder;
import org.l2jmobius.gameserver.model.residences.ClanHall;
import org.l2jmobius.gameserver.network.serverpackets.ConfirmDlg;

public class DoorAction implements IActionHandler
{
	@Override
	public boolean action(Player player, WorldObject target, boolean interact)
	{
		// Check if the Player already target the Npc
		if (player.getTarget() != target)
		{
			player.setTarget(target);
		}
		else if (interact)
		{
			final Door door = (Door) target;
			final ClanHall clanHall = ClanHallData.getInstance().getClanHallByDoorId(door.getId());
			// MyTargetSelected my = new MyTargetSelected(getObjectId(), player.getLevel());
			// player.sendPacket(my);
			if (target.isAutoAttackable(player))
			{
				if (Math.abs(player.getZ() - target.getZ()) < 400)
				{
					player.getAI().setIntention(CtrlIntention.AI_INTENTION_ATTACK, target);
				}
			}
			else if ((player.getClan() != null) && (clanHall != null) && (player.getClanId() == clanHall.getOwnerId()))
			{
				if (!door.isInsideRadius2D(player, Npc.INTERACTION_DISTANCE))
				{
					player.getAI().setIntention(CtrlIntention.AI_INTENTION_INTERACT, target);
				}
				else
				{
					player.addScript(new DoorRequestHolder(door));
					if (!door.isOpen())
					{
						player.sendPacket(new ConfirmDlg(1140));
					}
					else
					{
						player.sendPacket(new ConfirmDlg(1141));
					}
				}
			}
			else if ((player.getClan() != null) && (((Door) target).getFort() != null) && (player.getClan() == ((Door) target).getFort().getOwnerClan()) && ((Door) target).isOpenableBySkill() && !((Door) target).getFort().getSiege().isInProgress())
			{
				if (!((Creature) target).isInsideRadius2D(player, Npc.INTERACTION_DISTANCE))
				{
					player.getAI().setIntention(CtrlIntention.AI_INTENTION_INTERACT, target);
				}
				else
				{
					player.addScript(new DoorRequestHolder((Door) target));
					if (!((Door) target).isOpen())
					{
						player.sendPacket(new ConfirmDlg(1140));
					}
					else
					{
						player.sendPacket(new ConfirmDlg(1141));
					}
				}
			}
		}
		return true;
	}
	
	@Override
	public InstanceType getInstanceType()
	{
		return InstanceType.Door;
	}
}
