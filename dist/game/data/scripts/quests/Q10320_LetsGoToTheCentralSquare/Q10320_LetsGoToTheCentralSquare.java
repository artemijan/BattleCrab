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
package quests.Q10320_LetsGoToTheCentralSquare;

import org.l2jmobius.gameserver.model.actor.Creature;
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
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

/**
 * Let's Go To The Central Square (10320)
 * @author ivantotov, Gladicek
 */
public class Q10320_LetsGoToTheCentralSquare extends Quest
{
	// NPCs
	private static final int PANTHEON = 32972;
	private static final int THEODORE = 32975;
	
	// Zone
	private static final int TALKING_ISLAND_PRESENTATION_MOVIE_ZONE = 200034;
	
	// Misc
	private static final int MAX_LEVEL = 20;
	private static final String MOVIE_VAR = "TI_presentation_movie";
	
	public Q10320_LetsGoToTheCentralSquare()
	{
		super(10320);
		addStartNpc(PANTHEON);
		addTalkId(PANTHEON, THEODORE);
		addEnterZoneId(TALKING_ISLAND_PRESENTATION_MOVIE_ZONE);
		addCondMaxLevel(MAX_LEVEL, "32972-01a.html");
		addCondNotRace(Race.ERTHEIA, "32972-01b.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = null;
		switch (event)
		{
			case "32972-03.html":
			{
				qs.startQuest();
				qs.setCond(2); // arrow hack
				qs.setCond(1);
				player.sendPacket(new ExTutorialShowId(9));
				htmltext = event;
				break;
			}
			case "32972-02.htm":
			{
				htmltext = event;
				break;
			}
			case "32975-02.html":
			{
				if (qs.isStarted())
				{
					addExpAndSp(player, 84, 5);
					qs.exitQuest(false, true);
					htmltext = event;
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = null;
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = npc.getId() == PANTHEON ? "32972-01.htm" : "32975-04.html";
				break;
			}
			case State.STARTED:
			{
				htmltext = npc.getId() == PANTHEON ? "32972-04.html" : "32975-01.html";
				break;
			}
			case State.COMPLETED:
			{
				htmltext = npc.getId() == PANTHEON ? "32972-05.html" : "32975-03.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onEnterZone(Creature creature, ZoneType zone)
	{
		if (creature.isPlayer())
		{
			final Player player = creature.asPlayer();
			if (player.getVariables().getBoolean(MOVIE_VAR, false))
			{
				if (player.getLevel() <= MAX_LEVEL)
				{
					final QuestState qs = getQuestState(player, false);
					playMovie(player, ((qs != null) && qs.isStarted()) ? Movie.SI_ILLUSION_02_QUE : Movie.SI_ILLUSION_01_QUE);
				}
				
				player.getVariables().remove(MOVIE_VAR);
			}
		}
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
}
