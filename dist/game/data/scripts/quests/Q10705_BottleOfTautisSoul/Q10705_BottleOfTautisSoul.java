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
package quests.Q10705_BottleOfTautisSoul;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10384_AnAudienceWithTauti.Q10384_AnAudienceWithTauti;

/**
 * Bottle of Tauti's Soul (10705)
 * @URL http://l2on.net/en/?c=quests&id=10705&game=1
 * @author Gigi
 */
public class Q10705_BottleOfTautisSoul extends Quest
{
	// NPCs
	private static final int FERGASON = 33681;
	
	// Item
	private static final int BOTTLE_OF_TAUTIS_SOUL = 35295;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q10705_BottleOfTautisSoul()
	{
		super(10705);
		addStartNpc(FERGASON);
		addTalkId(FERGASON);
		addCondMinLevel(MIN_LEVEL, "33681-00.html");
		addCondCompletedQuest(Q10384_AnAudienceWithTauti.class.getSimpleName(), "33681-00.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		final QuestState qs1 = player.getQuestState(Q10384_AnAudienceWithTauti.class.getSimpleName());
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "33681-02.html":
			case "33681-03.html":
			case "33681-04.html":
			{
				htmltext = event;
				break;
			}
			case "33681-05.html":
			{
				qs.startQuest();
				break;
			}
			case "33681-06.html":
			{
				if (qs.isCond(1) && (getQuestItemsCount(player, BOTTLE_OF_TAUTIS_SOUL) >= 1))
				{
					takeItems(player, BOTTLE_OF_TAUTIS_SOUL, 1);
					qs1.setState(State.CREATED);
					qs1.setMemoState(1);
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
				if (getQuestItemsCount(player, BOTTLE_OF_TAUTIS_SOUL) >= 1)
				{
					htmltext = "33681-01.html";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "33681-05.html";
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
