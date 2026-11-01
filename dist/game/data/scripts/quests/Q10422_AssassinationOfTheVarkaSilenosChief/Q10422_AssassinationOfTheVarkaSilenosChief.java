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
package quests.Q10422_AssassinationOfTheVarkaSilenosChief;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10421_AssassinationOfTheVarkaSilenosCommander.Q10421_AssassinationOfTheVarkaSilenosCommander;

/**
 * Assassination of the Varka Silenos Chief (10422)
 * @author Stayway
 */
public class Q10422_AssassinationOfTheVarkaSilenosChief extends Quest
{
	// NPCs
	private static final int HANSEN = 33853;
	private static final int CHIEF_HORUS = 27503;
	private static final int KAMPF = 27516;
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 80;
	
	public Q10422_AssassinationOfTheVarkaSilenosChief()
	{
		super(10422);
		addStartNpc(HANSEN);
		addTalkId(HANSEN);
		addKillId(CHIEF_HORUS);
		addSpawnId(KAMPF);
		addCondNotRace(Race.ERTHEIA, "33853-08.html");
		addCondInCategory(CategoryType.FIGHTER_GROUP, "33853-09.htm");
		addCondMinLevel(MIN_LEVEL, "33853-09.htm");
		addCondMaxLevel(MAX_LEVEL, "33853-09.htm");
		addCondCompletedQuest(Q10421_AssassinationOfTheVarkaSilenosCommander.class.getSimpleName(), "33853-09.htm");
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
			case "33853-02.htm":
			case "33853-04.html":
			{
				htmltext = event;
				break;
			}
			case "33853-03.htm":
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
						addExpAndSp(player, 351479151, 1839);
					}
					
					htmltext = "33853-07.html";
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
		if (npc.getId() == HANSEN)
		{
			if (qs.getState() == State.CREATED)
			{
				htmltext = "33853-01.htm";
			}
			else if (qs.getState() == State.STARTED)
			{
				if (qs.isCond(1))
				{
					htmltext = "33853-05.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "33853-06.html";
				}
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
		
		if ((qs != null) && qs.isCond(2))
		{
			addSpawn(KAMPF, 105626, -43053, -1721, 0, true, 60000);
		}
	}
}
