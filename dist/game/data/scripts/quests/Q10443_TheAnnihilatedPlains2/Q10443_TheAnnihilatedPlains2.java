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
package quests.Q10443_TheAnnihilatedPlains2;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10442_TheAnnihilatedPlains1.Q10442_TheAnnihilatedPlains1;

/**
 * The Annihilated Plains - 2 (10443)
 * @URL https://l2wiki.com/The_Annihilated_Plains_-_2
 * @author Gigi
 */
public class Q10443_TheAnnihilatedPlains2 extends Quest
{
	// NPCs
	private static final int TUSKA = 33839;
	private static final int REFUGEE_CORPSE = 33837;
	private static final int FALK = 33843;
	
	// Item
	private static final int REFUGEES_NEACKLES = 36678;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	public Q10443_TheAnnihilatedPlains2()
	{
		super(10443);
		addStartNpc(TUSKA);
		addTalkId(TUSKA, REFUGEE_CORPSE, FALK);
		addFirstTalkId(REFUGEE_CORPSE);
		registerQuestItems(REFUGEES_NEACKLES);
		addCondMinLevel(MIN_LEVEL, "33839-00.htm");
		addCondCompletedQuest(Q10442_TheAnnihilatedPlains1.class.getSimpleName(), "33839-00.htm");
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
			case "33839-02.htm":
			{
				htmltext = event;
				break;
			}
			case "33839-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33843-02.htm":
			{
				if (qs.isCond(2))
				{
					addExpAndSp(player, 308731500, 74095);
					giveItems(player, 30357, 50);
					giveItems(player, 30358, 50);
					giveItems(player, 34609, 10000);
					giveItems(player, 34616, 10000);
					giveItems(player, 37018, 1);
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
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == TUSKA)
				{
					htmltext = "33839-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case TUSKA:
					{
						if (qs.isCond(1))
						{
							htmltext = "33839-04.html";
						}
						break;
					}
					case REFUGEE_CORPSE:
					{
						if (qs.isStarted() && qs.isCond(1))
						{
							giveItems(player, REFUGEES_NEACKLES, 1);
							qs.setCond(2, true);
							htmltext = "33837-01.html";
						}
						break;
					}
					case FALK:
					{
						if (qs.isStarted() && qs.isCond(2))
						{
							htmltext = "33843-01.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "33837.html";
	}
}
