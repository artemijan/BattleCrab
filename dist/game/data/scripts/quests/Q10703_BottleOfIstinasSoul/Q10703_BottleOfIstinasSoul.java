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
package quests.Q10703_BottleOfIstinasSoul;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q00150_ExtremeChallengePrimalMotherResurrected.Q00150_ExtremeChallengePrimalMotherResurrected;

/**
 * Bottle of Istina's Soul (10703)
 * @URL http://l2on.net/en/?c=quests&id=10703&game=1
 * @author Gigi
 */
public class Q10703_BottleOfIstinasSoul extends Quest
{
	// NPCs
	private static final int RUMIESE = 33293;
	
	// Item
	private static final int ISTINAS_SOUL_BOTTLE = 34883;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q10703_BottleOfIstinasSoul()
	{
		super(10703);
		addStartNpc(RUMIESE);
		addTalkId(RUMIESE);
		addCondMinLevel(MIN_LEVEL, "33293-00.html");
		addCondCompletedQuest(Q00150_ExtremeChallengePrimalMotherResurrected.class.getSimpleName(), "33293-00.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		final QuestState qs1 = player.getQuestState(Q00150_ExtremeChallengePrimalMotherResurrected.class.getSimpleName());
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "33293-02.html":
			case "33293-03.html":
			case "33293-04.html":
			{
				htmltext = event;
				break;
			}
			case "33293-05.html":
			{
				qs.startQuest();
				break;
			}
			case "33293-06.html":
			{
				if (qs.isCond(1) && (getQuestItemsCount(player, ISTINAS_SOUL_BOTTLE) >= 1))
				{
					takeItems(player, ISTINAS_SOUL_BOTTLE, 1);
					qs1.setState(State.CREATED);
					qs.exitQuest(false, true);
					htmltext = event;
				}
				else
				{
					htmltext = getNoQuestMsg(player);
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
				if (getQuestItemsCount(player, ISTINAS_SOUL_BOTTLE) >= 1)
				{
					htmltext = "33293-01.html";
				}
				else
				{
					htmltext = getNoQuestMsg(player);
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "33293-05.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getNoQuestMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
}
