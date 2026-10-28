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
package quests.Q00019_GoToThePastureland;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Go to the Pastureland (19)
 * @author malyelfik
 */
public class Q00019_GoToThePastureland extends Quest
{
	// NPCs
	private static final int VLADIMIR = 31302;
	private static final int TUNATUN = 31537;
	
	// Items
	private static final int VEAL = 15532;
	private static final int YOUNG_WILD_BEAST_MEAT = 7547;
	
	// Misc
	private static final int MIN_LEVEL = 82;
	
	public Q00019_GoToThePastureland()
	{
		super(19);
		addStartNpc(VLADIMIR);
		addTalkId(VLADIMIR, TUNATUN);
		registerQuestItems(VEAL, YOUNG_WILD_BEAST_MEAT);
		addCondMinLevel(MIN_LEVEL, "31302-03.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		if (event.equalsIgnoreCase("31302-02.htm"))
		{
			qs.startQuest();
			giveItems(player, VEAL, 1);
		}
		else if (event.equalsIgnoreCase("31537-02.htm"))
		{
			if (hasQuestItems(player, YOUNG_WILD_BEAST_MEAT))
			{
				giveAdena(player, 50000, true);
				addExpAndSp(player, 136766, 59); // TODO: Retail like SP value
				qs.exitQuest(false, true);
				htmltext = event;
			}
			else if (hasQuestItems(player, VEAL))
			{
				giveAdena(player, 299928, true);
				addExpAndSp(player, 1_456_218, 349);
				qs.exitQuest(false, true);
				htmltext = event;
			}
			else
			{
				htmltext = "31537-03.html";
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		if (npc.getId() == VLADIMIR)
		{
			switch (qs.getState())
			{
				case State.CREATED:
				{
					htmltext = "31302-01.html";
					break;
				}
				case State.STARTED:
				{
					htmltext = "31302-04.html";
					break;
				}
				case State.COMPLETED:
				{
					htmltext = getAlreadyCompletedMsg(player);
					break;
				}
			}
		}
		else if ((npc.getId() == TUNATUN) && (qs.isCond(1)))
		{
			htmltext = "31537-01.html";
		}
		
		return htmltext;
	}
}
