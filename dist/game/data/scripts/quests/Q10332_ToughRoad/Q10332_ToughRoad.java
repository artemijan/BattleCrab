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
package quests.Q10332_ToughRoad;

import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.Q10331_StartOfFate.Q10331_StartOfFate;

/**
 * Tough Road (10332)
 * @author St3eT, Trevor The Third
 */
public class Q10332_ToughRoad extends Quest
{
	// NPCs
	private static final int KAKAI = 30565;
	private static final int BATHIS = 30332;
	
	// Misc
	private static final int MIN_LEVEL = 20;
	private static final int MAX_LEVEL = 40;
	private static final int ZONE_ID = 12016;
	private static final String MOVIE_VAR = "Q10332_MOVIE";
	
	public Q10332_ToughRoad()
	{
		super(10332);
		addStartNpc(KAKAI);
		addTalkId(KAKAI, BATHIS);
		addEnterZoneId(ZONE_ID);
		addCondNotRace(Race.ERTHEIA, "30565-05.html");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "30565-04.html");
		addCondCompletedQuest(Q10331_StartOfFate.class.getSimpleName(), "30565-04.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			if (event.equals("SCREEN_MSG"))
			{
				showOnScreenMsg(player, NpcStringId.PA_AGRIO_LORD_KAKAI_IS_CALLING_FOR_YOU, ExShowScreenMessage.TOP_CENTER, 10000);
			}
			
			return null;
		}
		
		String htmltext = null;
		switch (event)
		{
			case "30332-02.html":
			{
				htmltext = event;
				break;
			}
			case "30565-02.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30332-03.html":
			{
				if (qs.isCond(1))
				{
					addExpAndSp(player, 42250, 20);
					qs.exitQuest(false, true);
					player.getVariables().remove(MOVIE_VAR);
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
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		if (npc.getId() == KAKAI)
		{
			switch (qs.getState())
			{
				case State.CREATED:
				{
					htmltext = "30565-01.htm";
					break;
				}
				case State.STARTED:
				{
					htmltext = "30565-06.html";
					break;
				}
				case State.COMPLETED:
				{
					htmltext = "30565-03.html";
					break;
				}
			}
		}
		else if (npc.getId() == BATHIS)
		{
			if (qs.getState() == State.STARTED)
			{
				htmltext = "30332-01.html";
			}
			else if (qs.getState() == State.COMPLETED)
			{
				htmltext = "30332-04.html";
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
			final QuestState qs = getQuestState(player, false);
			final QuestState st10331 = player.getQuestState(Q10331_StartOfFate.class.getSimpleName());
			if (((qs == null) || qs.isCreated()) && (player.getLevel() >= MIN_LEVEL) && (player.getLevel() <= MAX_LEVEL) && (st10331 != null) && st10331.isCompleted() && !player.getVariables().getBoolean(MOVIE_VAR, false))
			{
				player.getVariables().set(MOVIE_VAR, true);
				playMovie(player, Movie.SI_ILLUSION_04_QUE);
				startQuestTimer("SCREEN_MSG", 11000, null, player);
			}
		}
	}
}
