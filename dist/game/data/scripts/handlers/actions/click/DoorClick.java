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
package handlers.actions.click;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.data.xml.ClanHallData;
import org.l2jmobius.gameserver.handler.IActionClickHandler;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.InstanceType;
import org.l2jmobius.gameserver.model.actor.holders.creature.DoorRequestHolder;
import org.l2jmobius.gameserver.model.actor.instance.Door;
import org.l2jmobius.gameserver.model.clan.Clan;
import org.l2jmobius.gameserver.model.residences.ClanHall;
import org.l2jmobius.gameserver.network.serverpackets.ConfirmDlg;

public class DoorClick implements IActionClickHandler
{
	@Override
	public boolean onAction(Player player, WorldObject target, boolean interact)
	{
		// Check if the Player already target the Npc
		if (player.getTarget() != target)
		{
			player.setTarget(target);
		}
		else if (interact)
		{
			final Door door = target.asDoor();
			final ClanHall clanHall = ClanHallData.getInstance().getClanHallByDoorId(door.getId());
			
			// MyTargetSelected my = new MyTargetSelected(getObjectId(), player.getLevel());
			// player.sendPacket(my);
			final Clan clan = player.getClan();
			if (target.isAutoAttackable(player))
			{
				if (Math.abs(player.getZ() - target.getZ()) < 400)
				{
					player.getAI().setIntention(Intention.ATTACK, target);
				}
			}
			else if ((clan != null) && (clanHall != null) && (player.getClanId() == clanHall.getOwnerId()))
			{
				if (!door.isInsideRadius2D(player, Npc.INTERACTION_DISTANCE))
				{
					player.getAI().setIntention(Intention.INTERACT, target);
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
			else if ((clan != null) && (target.asDoor().getFort() != null) && (clan == target.asDoor().getFort().getOwnerClan()) && target.asDoor().isOpenableBySkill() && !target.asDoor().getFort().getSiege().isInProgress())
			{
				if (!target.asCreature().isInsideRadius2D(player, Npc.INTERACTION_DISTANCE))
				{
					player.getAI().setIntention(Intention.INTERACT, target);
				}
				else
				{
					player.addScript(new DoorRequestHolder(target.asDoor()));
					if (!target.asDoor().isOpen())
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
