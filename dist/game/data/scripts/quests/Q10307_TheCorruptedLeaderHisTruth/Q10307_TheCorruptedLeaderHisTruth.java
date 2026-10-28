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
package quests.Q10307_TheCorruptedLeaderHisTruth;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10306_TheCorruptedLeader.Q10306_TheCorruptedLeader;

/**
 * The Corrupted Leader: His Truth (10307)
 * @URL https://l2wiki.com/The_Corrupted_Leader:_His_Truth
 * @VIDEO https://www.youtube.com/watch?v=MI5Hyu7TtLw
 * @author Gigi
 */
public class Q10307_TheCorruptedLeaderHisTruth extends Quest
{
	// NPCs
	private static final int NEOTI_MIMILEAD = 32895;
	private static final int NAOMI_KASHERON = 32896;
	private static final int[] BOSS =
	{
		25745,
		25747
	};
	private static final int ENCHANT_ARMOR_R = 17527;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	public Q10307_TheCorruptedLeaderHisTruth()
	{
		super(10307);
		addStartNpc(NAOMI_KASHERON);
		addTalkId(NAOMI_KASHERON, NEOTI_MIMILEAD);
		addKillId(BOSS);
		addCondMinLevel(MIN_LEVEL, "32896-03.html");
		addCondCompletedQuest(Q10306_TheCorruptedLeader.class.getSimpleName(), "32896-03.html");
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
			case "32896-07.html":
			case "32895-02.html":
			case "32895-03.html":
			{
				htmltext = event;
				break;
			}
			case "32896-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32896-08.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "32895-04.html":
			{
				if (player.getLevel() >= MIN_LEVEL)
				{
					addExpAndSp(player, 11779522, 2827);
					giveItems(player, ENCHANT_ARMOR_R, 5);
					qs.exitQuest(QuestType.ONE_TIME, true);
					htmltext = event;
				}
				else
				{
					htmltext = getNoQuestLevelRewardMsg(player);
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
		switch (npc.getId())
		{
			case NAOMI_KASHERON:
			{
				if (qs.isCreated())
				{
					htmltext = "32896-01.htm";
					break;
				}
				else if (qs.isCond(1))
				{
					htmltext = "32896-05.htm";
					break;
				}
				else if (qs.isCond(2))
				{
					htmltext = "32895-06.html";
					break;
				}
				else if (qs.isCompleted())
				{
					htmltext = "32896-02.html";
				}
				break;
			}
			case NEOTI_MIMILEAD:
			{
				if (qs.isCond(3))
				{
					htmltext = "32895-01.html";
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1) && ArrayUtil.contains(BOSS, npc.getId()))
		{
			qs.setCond(2, true);
		}
	}
}
