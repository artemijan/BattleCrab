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
package ai.areas.TalkingIsland.YeSegiraTeleportDevice;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerCreate;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.enums.Movie;

import quests.Q10365_ForTheSearchdogKing.Q10365_ForTheSearchdogKing;

/**
 * Ye Segira Teleport Device AI.
 * @author St3eT
 */
public class YeSegiraTeleportDevice extends Script
{
	// NPCs
	private static final int[] TELEPORT_DEVICES =
	{
		33180,
		33181,
		33182,
		33183,
		33185,
		33186,
		33187,
		33188,
		33189,
		33190,
		33191,
		33205,
		33192,
		33197,
	};
	
	// Locations
	private static final Map<String, Location> LOCATIONS = new HashMap<>();
	static
	{
		LOCATIONS.put("village", new Location(-114413, 252159, -1592));
		LOCATIONS.put("village2", new Location(-112529, 256741, -1456));
		LOCATIONS.put("observatory", new Location(-114675, 230171, -1648));
		LOCATIONS.put("easten_exit", new Location(-109294, 237397, -2928));
		LOCATIONS.put("western_exit", new Location(-122189, 241009, -2328));
		LOCATIONS.put("1_exploration_zone", new Location(-115005, 237383, -3088));
		LOCATIONS.put("2_exploration_zone", new Location(-118350, 233992, -2904));
		LOCATIONS.put("3_exploration_zone", new Location(-116303, 239062, -2736));
		LOCATIONS.put("4_exploration_zone", new Location(-112382, 238710, -2904));
		LOCATIONS.put("5_exploration_zone", new Location(-110980, 233774, -3200));
	}
	
	// Misc
	private static final String MOVIE_VAR = "TI_YESEGIRA_MOVIE";
	
	private YeSegiraTeleportDevice()
	{
		addFirstTalkId(TELEPORT_DEVICES);
		addStartNpc(TELEPORT_DEVICES);
		addTalkId(TELEPORT_DEVICES);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (LOCATIONS.containsKey(event))
		{
			player.teleToLocation(LOCATIONS.get(event), true);
			if (event.equals("observatory") && player.getVariables().getBoolean(MOVIE_VAR, false))
			{
				playMovie(player, Movie.SI_ILLUSION_03_QUE);
				player.getVariables().remove(MOVIE_VAR);
			}
			else if (event.equals("5_exploration_zone"))
			{
				final QuestState qs = player.getQuestState(Q10365_ForTheSearchdogKing.class.getSimpleName());
				if ((qs != null) && qs.isStarted() && qs.isCond(1))
				{
					final Quest quest10365 = ScriptManager.getInstance().getScript(Q10365_ForTheSearchdogKing.class.getSimpleName());
					if (quest10365 != null)
					{
						quest10365.notifyEvent("TELEPORT_TO_NEXT_STAGE", null, player);
					}
				}
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@RegisterEvent(EventType.ON_PLAYER_CREATE)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerCreate(OnPlayerCreate event)
	{
		final Player player = event.getPlayer();
		if (player.getRace() != Race.ERTHEIA)
		{
			player.getVariables().set(MOVIE_VAR, true);
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	public static void main(String[] args)
	{
		new YeSegiraTeleportDevice();
	}
}
