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
package quests.Q00586_MutatedCreatures;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Mutated Creatures (586)
 * @URL https://l2wiki.com/Mutated_Creatures
 * @author Dmitri
 */
public class Q00586_MutatedCreatures extends Quest
{
	// NPC
	private static final int NERUPA = 30370;
	
	// Monsters
	private static final int COCOON_DESTROYER = 19294; // Cocoon Destroyer (Violent) 93
	
	// Items
	private static final int COCOON_DESTROYER_SHELL = 48382;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	private static final int MAX_LEVEL = 100;
	
	public Q00586_MutatedCreatures()
	{
		super(586);
		addStartNpc(NERUPA);
		addTalkId(NERUPA);
		addKillId(COCOON_DESTROYER);
		registerQuestItems(COCOON_DESTROYER_SHELL);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "nolevel.html");
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
			case "30370-02.htm":
			case "30370-03.htm":
			{
				htmltext = event;
				break;
			}
			case "30370-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30370-07.html":
			{
				giveAdena(player, 559020, true);
				addExpAndSp(player, 646727130, 646710);
				qs.exitQuest(QuestType.DAILY, true);
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
				htmltext = "30370-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "30370-05.html";
				}
				else
				{
					htmltext = "30370-06.html";
				}
				break;
			}
			case State.COMPLETED:
			
			{
				if (qs.isNowAvailable())
				{
					qs.setState(State.CREATED);
					htmltext = "30370-01.htm";
				}
				else
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && giveItemRandomly(killer, npc, COCOON_DESTROYER_SHELL, 1, 25, 1, true))
		{
			qs.setCond(2, true);
		}
	}
}
