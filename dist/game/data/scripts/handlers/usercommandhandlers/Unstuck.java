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
package handlers.usercommandhandlers;

import static org.l2jmobius.gameserver.ai.CtrlIntention.AI_INTENTION_ACTIVE;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.handler.IUserCommandHandler;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.SkillCastingType;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ActionFailed;

/**
 * Unstuck user command.
 */
public class Unstuck implements IUserCommandHandler
{
	private static final int[] COMMAND_IDS =
	{
		52
	};
	
	@Override
	public boolean useUserCommand(int id, Player player)
	{
		if (player.isJailed())
		{
			player.sendMessage("You cannot use this function while you are jailed.");
			return false;
		}
		
		if (Config.FACTION_SYSTEM_ENABLED && !player.isGood() && !player.isEvil())
		{
			player.sendMessage("You cannot use this function while you are neutral.");
			return false;
		}
		
		final int unstuckTimer = (player.getAccessLevel().isGm() ? 1000 : Config.UNSTUCK_INTERVAL * 1000);
		
		if (player.isInOlympiadMode())
		{
			player.sendPacket(SystemMessageId.YOU_CANNOT_USE_THAT_SKILL_IN_A_OLYMPIAD_MATCH);
			return false;
		}
		
		if (player.isCastingNow(SkillCaster::isAnyNormalType) || player.isMovementDisabled() || player.isMuted() || player.isAlikeDead() || player.inObserverMode() || player.isCombatFlagEquipped())
		{
			return false;
		}
		
		final Skill escape = SkillData.getInstance().getSkill(2099, 1); // 5 minutes escape
		final Skill gmEscape = SkillData.getInstance().getSkill(2100, 1); // 1 second escape
		if (player.getAccessLevel().isGm())
		{
			if (gmEscape != null)
			{
				player.doCast(gmEscape);
				return true;
			}
			player.sendMessage("You use Escape: 1 second.");
		}
		else if ((Config.UNSTUCK_INTERVAL == 300) && (escape != null))
		{
			player.doCast(escape);
			return true;
		}
		else
		{
			final SkillCaster skillCaster = SkillCaster.castSkill(player, player.getTarget(), escape, null, SkillCastingType.NORMAL, false, false, unstuckTimer);
			if (skillCaster == null)
			{
				player.sendPacket(ActionFailed.get(SkillCastingType.NORMAL));
				player.getAI().setIntention(AI_INTENTION_ACTIVE);
				return false;
			}
			
			if (Config.UNSTUCK_INTERVAL > 100)
			{
				player.sendMessage("You use Escape: " + (unstuckTimer / 60000) + " minutes.");
			}
			else
			{
				player.sendMessage("You use Escape: " + (unstuckTimer / 1000) + " seconds.");
			}
		}
		return true;
	}
	
	@Override
	public int[] getUserCommandList()
	{
		return COMMAND_IDS;
	}
}
