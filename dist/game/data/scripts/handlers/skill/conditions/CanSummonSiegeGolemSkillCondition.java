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
package handlers.skill.conditions;

import org.l2jmobius.gameserver.managers.CastleManager;
import org.l2jmobius.gameserver.managers.FortManager;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.siege.Castle;
import org.l2jmobius.gameserver.model.siege.Fort;
import org.l2jmobius.gameserver.model.skill.ISkillCondition;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.SystemMessageId;

/**
 * @author Sdw
 */
public class CanSummonSiegeGolemSkillCondition implements ISkillCondition
{
	public CanSummonSiegeGolemSkillCondition(StatSet params)
	{
	}
	
	@Override
	public boolean canUse(Creature caster, Skill skill, WorldObject target)
	{
		if ((caster == null) || !caster.isPlayer())
		{
			return false;
		}
		
		final Player player = caster.asPlayer();
		boolean canSummonSiegeGolem = true;
		if (player.isAlikeDead() || player.isCursedWeaponEquipped() || (player.getClan() == null))
		{
			canSummonSiegeGolem = false;
		}
		
		final Castle castle = CastleManager.getInstance().getCastle(player);
		final Fort fort = FortManager.getInstance().getFort(player);
		if ((castle == null) && (fort == null))
		{
			canSummonSiegeGolem = false;
		}
		
		if (((fort != null) && (fort.getResidenceId() == 0)) || ((castle != null) && (castle.getResidenceId() == 0)))
		{
			player.sendPacket(SystemMessageId.INVALID_TARGET);
			canSummonSiegeGolem = false;
		}
		else if (((castle != null) && !castle.getSiege().isInProgress()) || ((fort != null) && !fort.getSiege().isInProgress()))
		{
			player.sendPacket(SystemMessageId.INVALID_TARGET);
			canSummonSiegeGolem = false;
		}
		else if ((player.getClanId() != 0) && (((castle != null) && (castle.getSiege().getAttackerClan(player.getClanId()) == null)) || ((fort != null) && (fort.getSiege().getAttackerClan(player.getClanId()) == null))))
		{
			player.sendPacket(SystemMessageId.INVALID_TARGET);
			canSummonSiegeGolem = false;
		}
		
		return canSummonSiegeGolem;
	}
}
