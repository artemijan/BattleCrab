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
package quests.Q00755_InNeedOfPetras;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;

/**
 * @author hlwrave
 */
public class Q00755_InNeedOfPetras extends Quest
{
	// NPCs
	private static final int AKU = 33671;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23213,
		23214,
		23227,
		23228,
		23229,
		23230,
		23215,
		23216,
		23217,
		23218,
		23231,
		23232,
		23233,
		23234,
		23237,
		23219
	};
	
	// Items
	private static final int AKUS_SUPPLY_BOX = 35550;
	private static final int ENERGY_OF_DESTRUCTION = 35562;
	private static final int PETRA = 34959;
	
	// Other
	private static final int MIN_LEVEL = 97;
	
	public Q00755_InNeedOfPetras()
	{
		super(755);
		addStartNpc(AKU);
		addTalkId(AKU);
		addKillId(MONSTERS);
		registerQuestItems(PETRA);
		addCondMinLevel(MIN_LEVEL, "sofa_aku_q0755_05.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		if (event.equals("sofa_aku_q0755_04.html"))
		{
			qs.startQuest();
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
			htmltext = "sofa_aku_q0755_01.htm";
		}
		else if (qs.isStarted())
		{
			if (qs.isCond(1))
			{
				htmltext = "sofa_aku_q0755_07.html";
			}
			else if (qs.isCond(2))
			{
				takeItems(player, PETRA, -1L);
				addExpAndSp(player, 570676680, 26102484);
				giveItems(player, AKUS_SUPPLY_BOX, 1);
				giveItems(player, ENERGY_OF_DESTRUCTION, 1);
				qs.exitQuest(QuestType.DAILY, true);
				htmltext = "sofa_aku_q0755_08.html";
			}
		}
		else if (qs.isCompleted())
		{
			htmltext = "sofa_aku_q0755_06.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && qs.isStarted() && giveItemRandomly(killer, npc, PETRA, 1, 50, 0.75, true))
		{
			qs.setCond(2);
		}
	}
}
