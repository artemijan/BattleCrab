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
package quests.Q00210_ObtainAWolfPet;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Obtain a Wolf Pet (210)
 * @author Stayway
 */
public class Q00210_ObtainAWolfPet extends Quest
{
	// NPCs
	private static final int LUNDY = 30827;
	private static final int BELLA = 30256;
	private static final int BYNN = 30335;
	private static final int SYDNIA = 30321;
	
	// Item
	private static final int WOLF_COLLAR = 2375;
	
	// Misc
	private static final int MIN_LEVEL = 15;
	
	public Q00210_ObtainAWolfPet()
	{
		super(210);
		addStartNpc(LUNDY);
		addTalkId(LUNDY, BELLA, BYNN, SYDNIA);
		addCondMinLevel(MIN_LEVEL, "no_level.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "30827-02.htm":
			case "30827-04.htm":
			case "30256-02.html":
			{
				htmltext = event;
				break;
			}
			case "30256-03.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2);
					htmltext = event;
				}
				break;
			}
			case "30827-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30335-02.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3);
					htmltext = event;
				}
				break;
			}
			case "30321-02.html":
			{
				if (qs.isCond(3))
				{
					qs.setCond(4);
					htmltext = event;
				}
				break;
			}
			case "30827-05.html":
			{
				if (qs.isCond(4))
				{
					rewardItems(player, WOLF_COLLAR, 1);
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
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == LUNDY)
				{
					htmltext = "30827-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case LUNDY:
					{
						if (qs.isCond(1))
						{
							htmltext = "30827-07.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "30827-07.html";
						}
						else if (qs.isCond(4))
						{
							htmltext = "30827-04.html";
						}
						break;
					}
					case BELLA:
					{
						if (qs.isCond(1))
						{
							htmltext = "30256-01.html";
						}
						break;
					}
					case BYNN:
					{
						if (qs.isCond(2))
						{
							htmltext = "30335-01.html";
						}
						break;
					}
					case SYDNIA:
					{
						if (qs.isCond(3))
						{
							htmltext = "30321-01.html";
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
}
