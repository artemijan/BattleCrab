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
package quests.Q10426_AssassinationOfTheKetraOrcCommander;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10425_TheKetraOrcSupporters.Q10425_TheKetraOrcSupporters;

/**
 * Assassination of the Ketra Orc Commander (10426)
 * @author Stayway
 */
public class Q10426_AssassinationOfTheKetraOrcCommander extends Quest
{
	// NPCs
	private static final int LUGONNES = 33852;
	private static final int COMMANDER_TAYR = 27500;
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 80;
	
	public Q10426_AssassinationOfTheKetraOrcCommander()
	{
		super(10426);
		addStartNpc(LUGONNES);
		addTalkId(LUGONNES);
		addKillId(COMMANDER_TAYR);
		addCondNotRace(Race.ERTHEIA, "33852-08.html");
		addCondInCategory(CategoryType.WIZARD_GROUP, "33852-09.htm");
		addCondMinLevel(MIN_LEVEL, "33852-09.htm");
		addCondMaxLevel(MAX_LEVEL, "33852-09.htm");
		addCondCompletedQuest(Q10425_TheKetraOrcSupporters.class.getSimpleName(), "33852-09.htm");
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
			case "33852-02.htm":
			case "33852-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33852-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "reward_9546":
			case "reward_9547":
			case "reward_9548":
			case "reward_9549":
			case "reward_9550":
			case "reward_9551":
			{
				if (qs.isCond(2))
				{
					final int stoneId = Integer.parseInt(event.replace("reward_", ""));
					qs.exitQuest(false, true);
					giveItems(player, stoneId, 15);
					giveStoryQuestReward(npc, player);
					if ((player.getLevel() >= MIN_LEVEL) && (player.getLevel() <= MAX_LEVEL))
					{
						addExpAndSp(player, 327446943, 1839);
					}
					
					htmltext = "33852-07.html";
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
		String htmltext = null;
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "33852-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = qs.isCond(1) ? "33852-05.html" : "33852-06.html";
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
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			qs.setCond(2, true);
		}
	}
}
