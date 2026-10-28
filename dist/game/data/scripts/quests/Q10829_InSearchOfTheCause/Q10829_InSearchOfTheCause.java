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
package quests.Q10829_InSearchOfTheCause;

import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * In Search of the Cause (10829)
 * @URL https://l2wiki.com/In_Search_of_the_Cause
 * @author Gigi
 */
public class Q10829_InSearchOfTheCause extends Quest
{
	// NPC
	private static final int SERESIN = 30657;
	private static final int BELAS = 34056;
	private static final int FERIN = 34054;
	private static final int CYPHONIA = 34055;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	private static final int SOE = 46158;
	
	public Q10829_InSearchOfTheCause()
	{
		super(10829);
		addStartNpc(SERESIN);
		addTalkId(SERESIN, BELAS, FERIN, CYPHONIA);
		addCreatureSeeId(BELAS);
		addCondMinLevel(MIN_LEVEL, "30657-00.htm");
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
			case "30657-02.htm":
			case "30657-03.htm":
			case "34054-02.html":
			{
				htmltext = event;
				break;
			}
			case "30657-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34054-03.html":
			{
				qs.setCond(2);
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34055-03.html":
			{
				giveItems(player, SOE, 1);
				addExpAndSp(player, 55369440, 132885);
				qs.exitQuest(false, true);
				htmltext = event;
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
				if (npc.getId() == SERESIN)
				{
					htmltext = "30657-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case SERESIN:
					{
						if (qs.isCond(1))
						{
							htmltext = "30657-05.html";
						}
						break;
					}
					case BELAS:
					{
						if (qs.isCond(2))
						{
							htmltext = "34056-01.html";
						}
						else if (qs.getCond() > 2)
						{
							htmltext = "34056-02.html";
						}
						break;
					}
					case FERIN:
					{
						if (qs.isCond(2))
						{
							htmltext = "34054-01.html";
						}
						else if (qs.getCond() > 2)
						{
							htmltext = "34054-04.html";
						}
						break;
					}
					case CYPHONIA:
					{
						if ((qs.getCond() > 1) && (qs.getCond() < 4))
						{
							htmltext = "34055-01.html";
						}
						else if (qs.isCond(4))
						{
							htmltext = "34055-02.html";
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
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer())
		{
			final QuestState qs = getQuestState(creature.asPlayer(), true);
			if ((qs != null) && qs.isCond(1) && creature.isPlayer())
			{
				qs.setCond(2, true);
			}
		}
	}
}
