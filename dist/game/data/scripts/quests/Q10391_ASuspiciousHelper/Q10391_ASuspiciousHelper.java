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
package quests.Q10391_ASuspiciousHelper;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * A Suspicious Helper (10391)
 * @author St3eT, Trevor The Third
 */
public class Q10391_ASuspiciousHelper extends Quest
{
	// NPCs
	private static final int ELI = 33858;
	private static final int CHEL = 33861;
	private static final int IASON = 33859;
	
	// Items
	private static final int CARD = 36707; // Forged Identification Card
	private static final int EXP_MATERTIAL = 36708; // Experimental Material
	
	// Misc
	private static final int MIN_LEVEL = 40;
	private static final int MAX_LEVEL = 46;
	
	public Q10391_ASuspiciousHelper()
	{
		super(10391);
		addStartNpc(ELI);
		addTalkId(ELI, CHEL, IASON);
		registerQuestItems(CARD, EXP_MATERTIAL);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33858-06.htm");
		addCondNotRace(Race.ERTHEIA, "33858-07.htm");
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
			case "33858-02.htm":
			case "33858-03.htm":
			case "33861-02.html":
			case "33859-02.html":
			case "33859-03.html":
			{
				htmltext = event;
				break;
			}
			case "33858-04.htm":
			{
				giveItems(player, CARD, 1);
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33861-03.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2, true);
					takeItems(player, CARD, -1);
					giveItems(player, EXP_MATERTIAL, 1);
					htmltext = event;
				}
				break;
			}
			case "33859-04.html":
			{
				if (qs.isCond(2))
				{
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					addExpAndSp(player, 388290, 93);
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
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == ELI)
				{
					htmltext = "33858-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case ELI:
					{
						if (qs.isCond(1))
						{
							htmltext = "33858-05.html";
						}
						break;
					}
					case CHEL:
					{
						if (qs.isCond(1))
						{
							htmltext = "33861-01.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "33861-04.html";
						}
						break;
					}
					case IASON:
					{
						if (qs.isCond(2))
						{
							htmltext = "33859-01.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (npc.getId() == ELI)
				{
					htmltext = getAlreadyCompletedMsg(player);
				}
				break;
			}
		}
		
		return htmltext;
	}
}
