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
package quests.Q10454_FinalEmbryoApostle;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Final Embryo Apostle (10454)
 * @URL https://l2wiki.com/Final_Embryo_Apostle
 * @author Dmitri
 */
public class Q10454_FinalEmbryoApostle extends Quest
{
	// NPCs
	private static final int ERDA = 34319;
	
	// Boss
	private static final int CAMILLE = 26236; // Camille - Inner Messiahs Castle
	
	// Item
	private static final int SCROLL_ENCHANT_R_GRADE_WEAPON = 19447;
	private static final int SCROLL_ENCHANT_R_GRADE_ARMOR = 19448;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	public Q10454_FinalEmbryoApostle()
	{
		super(10454);
		addStartNpc(ERDA);
		addTalkId(ERDA);
		addKillId(CAMILLE);
		addCondMinLevel(MIN_LEVEL, "34319-00.htm");
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
			case "34319-02.htm":
			case "34319-03.htm":
			case "34319-07.html":
			{
				htmltext = event;
				break;
			}
			case "34319-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34319-08.html":
			{
				if (qs.isCond(2))
				{
					giveItems(player, SCROLL_ENCHANT_R_GRADE_WEAPON, 1);
					giveItems(player, SCROLL_ENCHANT_R_GRADE_ARMOR, 1);
					addExpAndSp(player, 36255499714L, 87013199);
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
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "34319-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34319-05.html";
				}
				else
				{
					htmltext = "34319-06.html";
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
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		executeForEachPlayer(player, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			qs.setCond(2, true);
		}
	}
}
