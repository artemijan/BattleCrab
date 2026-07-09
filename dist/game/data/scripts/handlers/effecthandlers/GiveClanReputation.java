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
package handlers.effecthandlers;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.clan.ClanMember;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * Give Clan reputation effect implementation.
 * @author Mobius
 */
public class GiveClanReputation extends AbstractEffect
{
	private final int _reputation;
	
	public GiveClanReputation(StatSet params)
	{
		_reputation = params.getInt("reputation", 0);
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (!effector.isPlayer() || !effected.isPlayer() || effected.isAlikeDead() || (effector.getActingPlayer().getClan() == null))
		{
			return;
		}
		
		effector.getActingPlayer().getClan().addReputationScore(_reputation);
		
		for (ClanMember member : effector.getActingPlayer().getClan().getMembers())
		{
			if (member.isOnline())
			{
				final SystemMessage sm = new SystemMessage(SystemMessageId.YOUR_CLAN_HAS_ADDED_S1_POINT_S_TO_ITS_CLAN_REPUTATION);
				sm.addInt(_reputation);
				member.getPlayer().sendPacket(sm);
			}
		}
	}
}