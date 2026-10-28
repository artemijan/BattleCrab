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
package quests.Q10856_SuperionAppears;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Superion Appears (10856)
 * @URL https://l2wiki.com/Superion_Appears
 * @author Dmitri
 */
public class Q10856_SuperionAppears extends Quest
{
	// NPCs
	private static final int KEKROPUS = 34222;
	private static final int MELDINA = 32214;
	private static final int HISTY = 34243;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	public Q10856_SuperionAppears()
	{
		super(10856);
		addStartNpc(KEKROPUS);
		addTalkId(KEKROPUS, MELDINA, HISTY);
		addCondMinLevel(MIN_LEVEL, "level_check.htm");
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
			case "34222-02.htm":
			case "34222-06.htm":
			case "34214-02.htm":
			case "34222-05.htm":
			{
				htmltext = event;
				break;
			}
			case "34222-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34214-03.htm":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34222-04.htm":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "teleport":
			{
				qs.setCond(3, true);
				player.teleToLocation(79827, 152588, 2309);
				break;
			}
			case "finish":
			{
				htmltext = "34243-02.htm";
				giveAdena(player, 164122, true);
				addExpAndSp(player, 592571412, 1422162);
				qs.exitQuest(false, true);
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
				if (npc.getId() == KEKROPUS)
				{
					htmltext = "34222-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case KEKROPUS:
					{
						if (qs.getCond() == 1)
						{
							htmltext = "34222-09.htm";
						}
						else if (qs.getCond() == 2)
						{
							htmltext = "34222-07.htm";
						}
						else if (qs.getCond() == 3)
						{
							htmltext = "34222-08.htm";
						}
						break;
					}
					case MELDINA:
					{
						if (qs.getCond() == 1)
						{
							htmltext = "34214-01.htm";
						}
						else if (qs.getCond() == 2)
						{
							htmltext = "34214-04.htm";
						}
						break;
					}
					case HISTY:
					{
						if (qs.getCond() == 3)
						{
							htmltext = "34243-01.htm";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					htmltext = "34222-01.htm";
					break;
				}
				
				qs.setState(State.CREATED);
			}
		}
		
		return htmltext;
	}
}
