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
package quests.Q10571_StrategicReconciliation;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Strategic Reconciliation (10571).
 * @URL https://l2wiki.com/Strategic_Reconciliation
 * @Video https://www.youtube.com/watch?v=ULDdqe3aeHA
 * @author Gigi
 * @date 2019-08-27 - [22:53:03]
 */
public class Q10571_StrategicReconciliation extends Quest
{
	// NPCs
	private static final int TARTI = 34360;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		// Dimensional Crack
		23755, // Wandering Dead of the Dimension
		23757, // Lost Soul of the Dimension
		23759, // Roaming Vengeance of the Dimension
		23760, // Dimensional Vagabond
		23761, // Dimension Dissolver
		// Dimensional Rift quest monsters
		23806, // Wandering Dead of the Dimension
		23807, // Wandering Dimensional Spirit
		23808, // Lost Soul of the Dimension
		23809, // Lost Dimensional Evil Thoughts
		23810 // Roaming Vengeance of the Dimension
	};
	
	// Misc
	private static final int MIN_LEVEL = 95;
	private static final int MAX_LEVEL = 106;
	
	// Items
	private static final int DIMENSIONAL_TRACES = 48162;
	
	public Q10571_StrategicReconciliation()
	{
		super(10571);
		addStartNpc(TARTI);
		addTalkId(TARTI);
		addKillId(MONSTERS);
		registerQuestItems(DIMENSIONAL_TRACES);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "34360-00.htm");
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
			case "34360-02.htm":
			case "34360-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34360-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34360-07.html":
			{
				if (qs.isCond(2) && (player.getLevel() >= MIN_LEVEL))
				{
					addExpAndSp(player, 10_758_599_280L, 10_758_420);
					addFactionPoints(player, Faction.UNWORLDLY_VISITORS, 100);
					qs.exitQuest(QuestType.ONE_TIME, true);
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
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "34360-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.isCond(1)) ? "34360-05.html" : "34360-06.html";
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
		if ((qs != null) && (qs.isCond(1)) && giveItemRandomly(killer, npc, DIMENSIONAL_TRACES, 1, 50, 0.5, true))
		{
			qs.setCond(2, true);
		}
	}
}
