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
package quests.Q00149_PrimalMotherIstina;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Primal Mother, Istina (149)
 * @URL https://l2wiki.com/Primal_Mother,_Istina
 * @author Gigi
 */
public class Q00149_PrimalMotherIstina extends Quest
{
	// NPCs
	private static final int RUMIESE = 33293;
	private static final int ISTHINA_NORMAL = 29195;
	
	// Item
	private static final int SHILENS_MARK = 17589;
	private static final int ISTHINA_BRACELET = 19455;
	private static final int EAR = 17527;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	public Q00149_PrimalMotherIstina()
	{
		super(149);
		addStartNpc(RUMIESE);
		addTalkId(RUMIESE);
		addKillId(ISTHINA_NORMAL);
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
			case "reward_9546":
			case "reward_9547":
			case "reward_9548":
			case "reward_9549":
			case "reward_9550":
			case "reward_9551":
			{
				if (qs.isCond(2) && (getQuestItemsCount(player, SHILENS_MARK) >= 1))
				{
					final int stoneId = Integer.parseInt(event.replace("reward_", ""));
					takeItems(player, SHILENS_MARK, 1);
					addExpAndSp(player, 833065000, 199935);
					giveItems(player, ISTHINA_BRACELET, 1);
					giveItems(player, EAR, 10);
					giveItems(player, stoneId, 15);
					qs.exitQuest(false, true);
				}
				
				htmltext = "33293-07.html";
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
