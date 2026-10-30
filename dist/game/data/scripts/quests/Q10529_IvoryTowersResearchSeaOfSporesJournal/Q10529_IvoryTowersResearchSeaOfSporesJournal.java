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
package quests.Q10529_IvoryTowersResearchSeaOfSporesJournal;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Ivory Tower's Research - Sea of Spores Journal (10529)
 * @author Kazumi
 */
public final class Q10529_IvoryTowersResearchSeaOfSporesJournal extends Quest
{
	// NPCs
	private static final int BACON = 33846;
	
	// Items
	private static final int BLESSED_SCROLL_ARMOR_R_GRADE = 19448;
	private static final int FLOATING_SEA_JOURNAL = 48836;
	private static final int DISPERSING_ENERGY_OF_THE_FLOATING_SEA = 48837;
	
	// Monster
	private static final int[] MONSTERS_PART_1 =
	{
		24227, // Keros
		24228, // Falena
		24229, // Atrofe
		24230, // Nuba
		24231, // Torfedo
		29329, // Harane
		24234, // Lesatanas
		24235, // Arbor
		24236, // Tergus
		24237, // Skeletus
		24238, // Atrofine
	};
	private static final int ARMINUS = 29327;
	private static final int ARIMA = 29326;
	
	// Misc
	private static final int MIN_LEVEL = 106;
	private static final int ITEM_COUNT_PART_1 = 100;
	private static final int ITEM_COUNT_PART_2 = 2;
	
	public Q10529_IvoryTowersResearchSeaOfSporesJournal()
	{
		super(10529);
		addStartNpc(BACON);
		addTalkId(BACON);
		addKillId(MONSTERS_PART_1);
		addKillId(ARMINUS, ARIMA);
		addCondMinLevel(MIN_LEVEL, "disciple_bacon_q10529_02.htm");
		registerQuestItems(FLOATING_SEA_JOURNAL, DISPERSING_ENERGY_OF_THE_FLOATING_SEA);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		switch (event)
		{
			case "quest_accept":
			{
				qs.startQuest();
				htmltext = "disciple_bacon_q10529_05.htm";
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
				htmltext = "disciple_bacon_q10529_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
						htmltext = "disciple_bacon_q10529_06.htm";
						break;
					case 2:
						htmltext = "disciple_bacon_q10529_07.htm";
						break;
					case 3:
						htmltext = "disciple_bacon_q10529_09.htm";
						break;
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
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(BACON)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		// final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10529)
		{
			switch (reply)
			{
				case 1:
					showHtmlFile(player, "disciple_bacon_q10529_03.htm");
					break;
				case 2:
					showHtmlFile(player, "disciple_bacon_q10529_04.htm");
					break;
				case 3:
					qs.setCond(3, true);
					showHtmlFile(player, "disciple_bacon_q10529_08.htm");
					break;
				case 4:
					showHtmlFile(player, "disciple_bacon_q10529_10.htm");
					break;
				case 10:
				{
					if (qs.getCond() == 4)
					{
						if (player.getLevel() >= MIN_LEVEL)
						{
							qs.exitQuest(false, true);
							addExpAndSp(player, 99527685300L, 99527580);
							giveItems(player, BLESSED_SCROLL_ARMOR_R_GRADE, 1);
							showHtmlFile(player, "disciple_bacon_q10529_11.htm");
							break;
						}
						break;
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, true);
		
		if (qs != null)
		{
			if (qs.isCond(1))
			{
				if (ArrayUtil.contains(MONSTERS_PART_1, npc.getId()))
				{
					giveItemRandomly(player, npc, FLOATING_SEA_JOURNAL, 1, ITEM_COUNT_PART_1, 0.5, true);
					if ((getQuestItemsCount(player, FLOATING_SEA_JOURNAL) >= ITEM_COUNT_PART_1))
					{
						qs.setCond(2, true);
					}
				}
			}
			else if (qs.isCond(3))
			{
				switch (npc.getId())
				{
					case ARMINUS:
					{
						if ((qs.getMemoState() == 0) || (qs.getMemoState() == 2))
						{
							qs.setMemoState(1);
							giveItemRandomly(player, npc, DISPERSING_ENERGY_OF_THE_FLOATING_SEA, 1, ITEM_COUNT_PART_2, 1.0, true);
							if ((getQuestItemsCount(player, DISPERSING_ENERGY_OF_THE_FLOATING_SEA) >= ITEM_COUNT_PART_2))
							{
								qs.setCond(4, true);
								break;
							}
							break;
						}
						break;
					}
					case ARIMA:
					{
						if ((qs.getMemoState() == 0) || (qs.getMemoState() == 1))
						{
							qs.setMemoState(2);
							giveItemRandomly(player, npc, DISPERSING_ENERGY_OF_THE_FLOATING_SEA, 1, ITEM_COUNT_PART_2, 1.0, true);
							if ((getQuestItemsCount(player, DISPERSING_ENERGY_OF_THE_FLOATING_SEA) >= ITEM_COUNT_PART_2))
							{
								qs.setCond(4, true);
								break;
							}
							break;
						}
						break;
					}
				}
			}
		}
	}
}
