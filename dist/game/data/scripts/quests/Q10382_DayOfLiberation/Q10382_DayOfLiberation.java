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
package quests.Q10382_DayOfLiberation;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;

import quests.Q10381_ToTheSeedOfHellfire.Q10381_ToTheSeedOfHellfire;

/**
 * @author hlwrave
 */
public class Q10382_DayOfLiberation extends Quest
{
	// NPCs
	private static final int SIZRAK = 33669;
	private static final int TAUTI = 29236;
	
	// Items
	private static final int TAUTIS_BRACELET = 35293;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q10382_DayOfLiberation()
	{
		super(10382);
		addStartNpc(SIZRAK);
		addTalkId(SIZRAK);
		addKillId(TAUTI);
		addCondMinLevel(MIN_LEVEL, "sofa_sizraku_q10382_04.html");
		addCondCompletedQuest(Q10381_ToTheSeedOfHellfire.class.getSimpleName(), "sofa_sizraku_q10382_05.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "sofa_sizraku_q10382_02.htm":
			case "sofa_sizraku_q10382_09.html":
			{
				htmltext = event;
				break;
			}
			case "sofa_sizraku_q10382_03.html":
			{
				qs.startQuest();
				qs.set(Integer.toString(TAUTI), 0);
				htmltext = event;
				break;
			}
			case "sofa_sizraku_q10382_10.html":
			{
				addExpAndSp(player, 951127800, 435041400);
				giveAdena(player, 3256740, true);
				giveItems(player, TAUTIS_BRACELET, 1);
				qs.exitQuest(QuestType.ONE_TIME, true);
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
		if (qs.isCreated())
		{
			htmltext = "sofa_sizraku_q10382_01.htm";
		}
		else if (qs.isStarted())
		{
			if (qs.isCond(1))
			{
				htmltext = "sofa_sizraku_q10382_07.html";
			}
			else if (qs.isCond(2))
			{
				htmltext = "sofa_sizraku_q10382_08.html";
			}
		}
		else if (qs.isCompleted())
		{
			htmltext = "sofa_sizraku_q10382_06.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if (qs != null)
		{
			qs.setCond(2, true);
		}
	}
}
