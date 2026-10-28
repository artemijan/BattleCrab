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
package quests.Q00150_ExtremeChallengePrimalMotherResurrected;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Extreme Challenge: Primal Mother Resurrected (150)
 * @URL https://l2wiki.com/Extreme_Challenge:_Primal_Mother_Resurrected
 * @author Gigi
 */
public class Q00150_ExtremeChallengePrimalMotherResurrected extends Quest
{
	// NPCs
	private static final int RUMIESE = 33293;
	private static final int ISTHINA_EXTRIM = 29196;
	
	// Item
	private static final int SHILENS_MARK = 17589;
	private static final int BOTTLE_OF_ISTHINAS_SOUL = 34883;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q00150_ExtremeChallengePrimalMotherResurrected()
	{
		super(150);
		addStartNpc(RUMIESE);
		addTalkId(RUMIESE);
		addKillId(ISTHINA_EXTRIM);
		addCondMinLevel(MIN_LEVEL, "33293-00.htm");
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
			case "33293-02.htm":
			case "33293-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33293-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33293-07.html":
			{
				if (qs.isCond(2) && (getQuestItemsCount(player, SHILENS_MARK) >= 1))
				{
					takeItems(player, SHILENS_MARK, 1);
					giveItems(player, BOTTLE_OF_ISTHINAS_SOUL, 1);
					qs.exitQuest(false, true);
				}
				
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
				if (npc.getId() == RUMIESE)
				{
					htmltext = "33293-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "33293-05.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "33293-06.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "Complete.html";
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
		if ((qs != null) && qs.isStarted() && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			giveItems(player, SHILENS_MARK, 1);
			qs.setCond(2, true);
		}
	}
}
