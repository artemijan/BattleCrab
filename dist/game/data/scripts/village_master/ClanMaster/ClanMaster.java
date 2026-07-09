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
package village_master.ClanMaster;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.enums.SkillFinishType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.impl.creature.player.OnPlayerClanJoin;
import org.l2jmobius.gameserver.model.events.impl.creature.player.OnPlayerClanLeft;
import org.l2jmobius.gameserver.model.events.impl.creature.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.events.impl.creature.player.OnPlayerLogout;
import org.l2jmobius.gameserver.model.events.impl.creature.player.OnPlayerProfessionChange;
import org.l2jmobius.gameserver.model.skill.CommonSkill;

import ai.AbstractNpcAI;

/**
 * @author UnAfraid
 */
public class ClanMaster extends AbstractNpcAI
{
	// @formatter:off
	private static final int[] NPCS =
	{
		30026,30031,30037,30066,30070,30109,30115,30120,30154,30174,
		30175,30176,30187,30191,30195,30288,30289,30290,30297,30358,
		30373,30462,30474,30498,30499,30500,30503,30504,30505,30508,
		30511,30512,30513,30520,30525,30565,30594,30595,30676,30677,
		30681,30685,30687,30689,30694,30699,30704,30845,30847,30849,
		30854,30857,30862,30865,30894,30897,30900,30905,30910,30913,
	};
	// @formatter:on
	private static final Map<String, String> LEADER_REQUIRED = new HashMap<>();
	static
	{
		LEADER_REQUIRED.put("9000-03.htm", "9000-03-no.htm");
		LEADER_REQUIRED.put("9000-04.htm", "9000-04-no.htm");
		LEADER_REQUIRED.put("9000-05.htm", "9000-05-no.htm");
		LEADER_REQUIRED.put("9000-07.htm", "9000-07-no.htm");
		LEADER_REQUIRED.put("9000-12a.htm", "9000-07-no.htm");
		LEADER_REQUIRED.put("9000-12b.htm", "9000-07-no.htm");
		LEADER_REQUIRED.put("9000-13a.htm", "9000-07-no.htm");
		LEADER_REQUIRED.put("9000-13b.htm", "9000-07-no.htm");
		LEADER_REQUIRED.put("9000-14a.htm", "9000-07-no.htm");
		LEADER_REQUIRED.put("9000-14b.htm", "9000-07-no.htm");
		LEADER_REQUIRED.put("9000-15.htm", "9000-07-no.htm");
	}
	
	private ClanMaster()
	{
		addStartNpc(NPCS);
		addTalkId(NPCS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (LEADER_REQUIRED.containsKey(event) && !player.isClanLeader())
		{
			return LEADER_REQUIRED.get(event);
		}
		return event;
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		return "9000-01.htm";
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		if (player.isClanLeader())
		{
			player.getClan().getMembers().forEach(member ->
			{
				if (member.isOnline())
				{
					CommonSkill.CLAN_ADVENT.getSkill().applyEffects(member.getPlayer(), member.getPlayer());
				}
			});
		}
		else if ((player.getClan() != null) && player.getClan().getLeader().isOnline())
		{
			CommonSkill.CLAN_ADVENT.getSkill().applyEffects(player, player);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGOUT)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLogout(OnPlayerLogout event)
	{
		final Player player = event.getPlayer();
		if (player.isClanLeader())
		{
			player.getClan().getMembers().forEach(member ->
			{
				if (member.isOnline())
				{
					member.getPlayer().getEffectList().stopSkillEffects(SkillFinishType.REMOVED, CommonSkill.CLAN_ADVENT.getId());
				}
			});
		}
		if (player.getClan() != null)
		{
			player.getEffectList().stopSkillEffects(SkillFinishType.REMOVED, CommonSkill.CLAN_ADVENT.getId());
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_PROFESSION_CHANGE)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onProfessionChange(OnPlayerProfessionChange event)
	{
		final Player player = event.getPlayer();
		if (player.isClanLeader() || ((player.getClan() != null) && player.getClan().getLeader().isOnline()))
		{
			CommonSkill.CLAN_ADVENT.getSkill().applyEffects(player, player);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_CLAN_JOIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerClanJoin(OnPlayerClanJoin event)
	{
		final Player player = event.getClanMember().getPlayer();
		if (player.getClan().getLeader().isOnline())
		{
			CommonSkill.CLAN_ADVENT.getSkill().applyEffects(player, player);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_CLAN_LEFT)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerClanLeft(OnPlayerClanLeft event)
	{
		event.getClanMember().getPlayer().getEffectList().stopSkillEffects(SkillFinishType.REMOVED, CommonSkill.CLAN_ADVENT.getId());
	}
	
	public static void main(String[] args)
	{
		new ClanMaster();
	}
}
