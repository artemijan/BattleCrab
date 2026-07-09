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
package handlers.targethandlers.affectobject;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.handler.IAffectObjectHandler;
import org.l2jmobius.gameserver.model.CommandChannel;
import org.l2jmobius.gameserver.model.Party;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.clan.Clan;
import org.l2jmobius.gameserver.model.skill.targets.AffectObject;
import org.l2jmobius.gameserver.model.zone.ZoneId;

/**
 * Not Friend affect object implementation. Based on Gracia Final retail tests.<br>
 * Such are considered flagged/karma players (except party/clan/ally). Doesn't matter if in command channel.<br>
 * In arena such are considered clan/ally/command channel (except party).<br>
 * In peace zone such are considered monsters.<br>
 * Monsters consider such all players, npcs (Citizens, Guild Masters, Merchants, Guards, etc. except monsters). Doesn't matter if in peace zone or arena.
 * @author Nik
 */
public class NotFriend implements IAffectObjectHandler
{
	@Override
	public boolean checkAffectedObject(Creature creature, Creature target)
	{
		if (creature == target)
		{
			return false;
		}
		
		final Player player = creature.getActingPlayer();
		final Player targetPlayer = target.getActingPlayer();
		if ((player != null) && (targetPlayer != null))
		{
			// Same player.
			if (player == targetPlayer)
			{
				return false;
			}
			
			// Peace Zone.
			if (target.isInsidePeaceZone(player) && !player.getAccessLevel().allowPeaceAttack())
			{
				return false;
			}
			
			if (Config.ALT_COMMAND_CHANNEL_FRIENDS)
			{
				final CommandChannel playerCC = player.getCommandChannel();
				final CommandChannel targetCC = targetPlayer.getCommandChannel();
				if ((playerCC != null) && (targetCC != null) && (playerCC.getLeaderObjectId() == targetCC.getLeaderObjectId()))
				{
					return false;
				}
			}
			
			// Party (command channel doesn't make you friends).
			final Party party = player.getParty();
			final Party targetParty = targetPlayer.getParty();
			if ((party != null) && (targetParty != null) && (party.getLeaderObjectId() == targetParty.getLeaderObjectId()))
			{
				return false;
			}
			
			// Events.
			if (player.isOnEvent() && !player.isOnSoloEvent() && (player.getTeam() == target.getTeam()))
			{
				return false;
			}
			
			// Olympiad observer.
			if (targetPlayer.inObserverMode())
			{
				return false;
			}
			
			// Siege.
			if (target.isInsideZone(ZoneId.SIEGE))
			{
				// Players in the same siege side at the same castle are considered friends.
				return !player.isSiegeFriend(targetPlayer);
			}
			
			// Arena.
			if (creature.isInsideZone(ZoneId.PVP) && !creature.isInsideZone(ZoneId.SIEGE) && target.isInsideZone(ZoneId.PVP) && !target.isInsideZone(ZoneId.SIEGE))
			{
				return true;
			}
			
			// Duel.
			if (player.isInDuel() && targetPlayer.isInDuel() && (player.getDuelId() == targetPlayer.getDuelId()))
			{
				return true;
			}
			
			// Olympiad.
			if (player.isInOlympiadMode() && targetPlayer.isInOlympiadMode() && (player.getOlympiadGameId() == targetPlayer.getOlympiadGameId()))
			{
				return true;
			}
			
			// Clan.
			final Clan clan = player.getClan();
			final Clan targetClan = targetPlayer.getClan();
			if (clan != null)
			{
				if (clan == targetClan)
				{
					return false;
				}
				
				// War
				if ((targetClan != null) && clan.isAtWarWith(targetClan) && targetClan.isAtWarWith(clan))
				{
					return true;
				}
			}
			
			// Alliance.
			if ((player.getAllyId() != 0) && (player.getAllyId() == targetPlayer.getAllyId()))
			{
				return false;
			}
			
			// Auto play target mode check.
			if (player.isAutoPlaying() && ((targetPlayer.getPvpFlag() == 0) || (targetPlayer.getReputation() > -1)))
			{
				final int targetMode = player.getAutoPlaySettings().getNextTargetMode();
				if ((targetMode != 0 /* Any Target */) && (targetMode != 2 /* Characters */))
				{
					return false;
				}
			}
			
			// At this point summon should be prevented from attacking friendly targets.
			if (creature.isSummon() && (target == creature.getTarget()))
			{
				return true;
			}
			
			// By default any flagged/PK player is considered enemy.
			return (targetPlayer.getPvpFlag() > 0) || (targetPlayer.getReputation() < 0);
		}
		
		return target.isAutoAttackable(creature);
	}
	
	@Override
	public Enum<AffectObject> getAffectObjectType()
	{
		return AffectObject.NOT_FRIEND;
	}
}
