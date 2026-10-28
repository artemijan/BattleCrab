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
package quests.Q00745_TheOutlawsAreIncoming;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * The Outlaws are Incoming (745)
 * @author Kazumi
 */
public class Q00745_TheOutlawsAreIncoming extends Quest
{
	// Items
	private static final int GARGOYLE_FRAGMENT = 47047;
	private static final int BASILISK_SCALE = 47048;
	private static final int MAHUM_ID_TAG = 47049;
	
	// NPCs
	private static final int FLUTTER = 30677;
	private static final int KELIOS = 33862;
	private static final int MOUEN = 30196;
	// @formatter:off
	private static final int[][] MONSTER_LIST =
	{
		// npcId, drop
		{20241, GARGOYLE_FRAGMENT}, // Hunter Gargoyle
		// {20286, GARGOYLE_FRAGMENT}, // Hunter Gargoyle
		{20573, BASILISK_SCALE}, // Tarlk Basilisk
		{20574, BASILISK_SCALE}, // Elder Tarlk Basilisk
		// {, MAHUM_ID_TAG}, // Ol Mahum Shaman ID?
		{21261, MAHUM_ID_TAG}, // Ol Mahum Transcender
		{21262, MAHUM_ID_TAG}, // Ol Mahum Transcender
		{21263, MAHUM_ID_TAG}, // Ol Mahum Transcender
		{21264, MAHUM_ID_TAG}, // Ol Mahum Transcender
	};
	// @formatter:on
	
	// Misc
	private static final int MIN_LEVEL = 46;
	private static final int MAX_LEVEL = 55;
	private static final int GARGOYLE_FRAGMENT_COUNT = 20;
	private static final int BASILISK_SCALE_COUNT = 50;
	private static final int MAHUM_ID_TAG_COUNT = 80;
	
	public Q00745_TheOutlawsAreIncoming()
	{
		super(745);
		addStartNpc(FLUTTER);
		addTalkId(FLUTTER, KELIOS, MOUEN);
		for (int[] mob : MONSTER_LIST)
		{
			addKillId(mob[0]);
		}
		
		registerQuestItems(GARGOYLE_FRAGMENT, BASILISK_SCALE, MAHUM_ID_TAG);
		addCondIsSubClassActive("head_blacksmith_flutter_q0745_03.htm");
		addCondMinLevel(MIN_LEVEL, "head_blacksmith_flutter_q0745_02.htm");
		addCondMaxLevel(MAX_LEVEL, "head_blacksmith_flutter_q0745_02.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = event;
		switch (event)
		{
			case "head_blacksmith_flutter_q0745_04.htm":
			case "head_blacksmith_flutter_q0745_05.htm":
			case "kelios_q0745_02.htm":
			{
				// htmltext = event;
				break;
			}
			case "head_blacksmith_flutter_q0745_06.htm":
			{
				qs.startQuest();
				break;
			}
			case "kelios_q0745_03.htm":
			{
				if (player.isSubClassActive())
				{
					qs.setCond(2, true);
				}
				break;
			}
			case "mouen_q0745_03.htm":
			{
				if (player.isSubClassActive() && qs.isCond(3))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						qs.exitQuest(false, true);
						takeItems(player, GARGOYLE_FRAGMENT, GARGOYLE_FRAGMENT_COUNT);
						takeItems(player, BASILISK_SCALE, BASILISK_SCALE_COUNT);
						takeItems(player, MAHUM_ID_TAG, MAHUM_ID_TAG_COUNT);
						addExpAndSp(player, 39862785, 3750);
						break;
					}
					
					htmltext = getNoQuestLevelRewardMsg(player);
					break;
				}
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
				if (npc.getId() == FLUTTER)
				{
					htmltext = "head_blacksmith_flutter_q0745_01.htm";
					break;
				}
				
				// fallthrou
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case FLUTTER:
					{
						if (qs.getCond() == 1)
						{
							htmltext = "head_blacksmith_flutter_q0745_07.htm";
						}
						break;
					}
					case KELIOS:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "kelios_q0745_01.htm";
								break;
							}
							case 2:
							{
								htmltext = "kelios_q0745_04.htm";
								break;
							}
							case 3:
							{
								htmltext = "kelios_q0745_05.htm";
								break;
							}
						}
						break;
					}
					case MOUEN:
					{
						switch (qs.getCond())
						{
							case 2:
							{
								htmltext = "mouen_q0745_01.htm";
								break;
							}
							case 3:
							{
								htmltext = "mouen_q0745_02.htm";
								break;
							}
						}
						break;
					}
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
		final QuestState qs = getQuestState(player, false);
		if (qs != null)
		{
			if (player.isSubClassActive() && qs.isCond(2))
			{
				final int npcId = npc.getId();
				for (int[] dropInfo : MONSTER_LIST)
				{
					if (dropInfo[0] == npcId)
					{
						final int itemId = dropInfo[1];
						switch (itemId)
						{
							case GARGOYLE_FRAGMENT:
							{
								if ((getQuestItemsCount(player, GARGOYLE_FRAGMENT) < GARGOYLE_FRAGMENT_COUNT))
								{
									giveItemRandomly(player, npc, itemId, 1, GARGOYLE_FRAGMENT_COUNT, 1.0, true);
									break;
								}
								break;
							}
							case BASILISK_SCALE:
							{
								if ((getQuestItemsCount(player, BASILISK_SCALE) < BASILISK_SCALE_COUNT))
								{
									giveItemRandomly(player, npc, itemId, 1, BASILISK_SCALE_COUNT, 1.0, true);
									break;
								}
								break;
							}
							case MAHUM_ID_TAG:
							{
								if ((getQuestItemsCount(player, MAHUM_ID_TAG) < MAHUM_ID_TAG_COUNT))
								{
									giveItemRandomly(player, npc, itemId, 1, MAHUM_ID_TAG_COUNT, 1.0, true);
									break;
								}
								break;
							}
						}
						
						if ((getQuestItemsCount(player, GARGOYLE_FRAGMENT) >= GARGOYLE_FRAGMENT_COUNT) //
							&& (getQuestItemsCount(player, BASILISK_SCALE) >= BASILISK_SCALE_COUNT) //
							&& (getQuestItemsCount(player, MAHUM_ID_TAG) >= MAHUM_ID_TAG_COUNT))
						{
							qs.setCond(3, true);
						}
					}
				}
			}
		}
	}
}
