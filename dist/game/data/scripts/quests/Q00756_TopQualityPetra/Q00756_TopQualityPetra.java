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
package quests.Q00756_TopQualityPetra;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * @author hlwrave
 */
public class Q00756_TopQualityPetra extends Quest
{
	// NPCs
	private static final int AKU = 33671;
	
	// Items
	private static final int AKU_MARK = 34910;
	private static final int TOP_QUALITY_PETRA = 35703;
	private static final int ZAHAK_PETRA = 35702;
	
	// Other
	private static final int MIN_LEVEL = 97;
	
	public Q00756_TopQualityPetra()
	{
		super(756);
		addTalkId(AKU);
		addItemTalkId(ZAHAK_PETRA);
		registerQuestItems(TOP_QUALITY_PETRA);
		addCondMinLevel(MIN_LEVEL, "sofa_aku_q0755_05.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		
		switch (event)
		{
			case "petra_of_zahaq_q0756_03.html":
			{
				qs.startQuest();
				takeItems(player, ZAHAK_PETRA, 1);
				giveItems(player, TOP_QUALITY_PETRA, 1);
				break;
			}
			case "sofa_aku_q0756_02.html":
			{
				takeItems(player, TOP_QUALITY_PETRA, -1);
				addExpAndSp(player, 570676680, 26102484);
				giveItems(player, AKU_MARK, 1);
				qs.exitQuest(QuestType.DAILY, true);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onItemTalk(Item item, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		boolean startQuest = false;
		switch (qs.getState())
		{
			case State.CREATED:
			{
				startQuest = true;
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					qs.setState(State.CREATED);
					startQuest = true;
				}
				break;
			}
		}
		
		if (startQuest)
		{
			if (player.getLevel() >= MIN_LEVEL)
			{
				qs.startQuest();
				takeItems(player, ZAHAK_PETRA, 1);
				giveItems(player, TOP_QUALITY_PETRA, 1);
				htmltext = "petra_of_zahaq_q0756_03.html";
			}
			else
			{
				htmltext = "petra_of_zahaq_q0756_02.htm";
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext; // = getNoQuestMsg(player);
		if (qs.isCond(1) && hasQuestItems(player, TOP_QUALITY_PETRA))
		{
			htmltext = "sofa_aku_q0756_01.html";
		}
		else
		{
			htmltext = "sofa_aku_q0756_03.html";
		}
		
		return htmltext;
	}
}
