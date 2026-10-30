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
package quests.Q10524_TheAssassinationOfTheVarkaSilenosCommanderChief;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

import quests.Q10523_TheAssassinationOfTheVarkaSilenosCommander.Q10523_TheAssassinationOfTheVarkaSilenosCommander;

/**
 * The Assassination of the Varka Silenos Commander Chief (10524)
 * @author Gigi
 * @date 2017-11-18 - [14:37:41]
 */
public class Q10524_TheAssassinationOfTheVarkaSilenosCommanderChief extends Quest
{
	// NPCs
	private static final int HANSEN = 33853;
	
	// Monsters
	private static final int VARKAS_CHIEF_HORUS = 27503;
	private static final int KAMPF = 27516;
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 80;
	
	public Q10524_TheAssassinationOfTheVarkaSilenosCommanderChief()
	{
		super(10524);
		addStartNpc(HANSEN);
		addTalkId(HANSEN);
		addKillId(VARKAS_CHIEF_HORUS);
		addCondRace(Race.ERTHEIA, "33853-00.html");
		addCondStart(p -> p.isInCategory(CategoryType.FIGHTER_GROUP), "33853-00a.htm");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33853-00.html");
		addCondCompletedQuest(Q10523_TheAssassinationOfTheVarkaSilenosCommander.class.getSimpleName(), "33853-00.html");
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
			case "33853-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33853-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33853-07.html":
			{
				if (qs.isCond(2))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 351479151, 1839);
						qs.exitQuest(QuestType.ONE_TIME, true);
						htmltext = event;
					}
					else
					{
						htmltext = getNoQuestLevelRewardMsg(player);
					}
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
				htmltext = "33853-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "33853-05.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "33853-06.html";
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
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, true);
		if ((qs != null) && qs.isCond(1))
		{
			npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.COME_KAMPF_PROTECT_ME);
			final Npc mob = addSpawn(KAMPF, npc, false, 120000);
			addAttackPlayerDesire(mob, killer);
			qs.setCond(2, true);
		}
	}
}
