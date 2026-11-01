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
package quests.Q10370_MenacingTimes;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Menacing Times (10370)
 * @URL https://l2wiki.com/Menacing_Times
 * @author Gigi
 */
public class Q10370_MenacingTimes extends Quest
{
	// NPCs
	private static final int ORVEN = 30857; // Human
	private static final int WINONIN = 30856; // Elf
	private static final int OLTRAN = 30862; // DarkElf
	private static final int LADANZA = 30865; // Orc
	private static final int FERRIS = 30847; // Dvarf
	private static final int BROME = 32221; // Kamael
	private static final int ANDREI = 31292;
	private static final int GERKENSHTEIN = 33648;
	
	// Monster's
	private static final int GRAVE_SCARAB = 21646;
	private static final int SCAVENGER_SCARAB = 21647;
	private static final int GRAVE_ANT = 21648;
	private static final int SCAVENGER_ANT = 21649;
	private static final int SHRINE_KNIGHT = 21650;
	private static final int SHRINE_ROYAL_GUARD = 21651;
	
	// Items
	private static final int REMNANT_ASHES = 34765;
	private static final ItemHolder ADENA = new ItemHolder(57, 479620);
	
	// Reward
	private static final int EXP_REWARD = 22451400;
	private static final int SP_REWARD = 5388;
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 81;
	
	public Q10370_MenacingTimes()
	{
		super(10370);
		addStartNpc(ORVEN, WINONIN, OLTRAN, LADANZA, FERRIS, BROME);
		addTalkId(ORVEN, WINONIN, OLTRAN, LADANZA, FERRIS, BROME, ANDREI, GERKENSHTEIN);
		addKillId(GRAVE_SCARAB, SCAVENGER_SCARAB, GRAVE_ANT, SCAVENGER_ANT, SHRINE_KNIGHT, SHRINE_ROYAL_GUARD);
		registerQuestItems(REMNANT_ASHES);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "no_level.htm");
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
			case "30857-02.htm":
			case "30856-02.htm":
			case "30862-02.htm":
			case "30865-02.htm":
			case "30847-02.htm":
			case "32221-02.htm":
			case "31292-02.html":
			{
				htmltext = event;
				break;
			}
			case "30857-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30856-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30862-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30865-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30847-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32221-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "31292-03.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "33648-02.html":
			{
				qs.setCond(0);
				qs.setCond(3, true);
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
				switch (npc.getId())
				{
					case ORVEN:
					{
						if ((player.getRace() == Race.HUMAN) && player.isInCategory(CategoryType.FOURTH_CLASS_GROUP))
						{
							htmltext = "30857-01.htm";
						}
						else
						{
							htmltext = "noRace.htm";
						}
						break;
					}
					case WINONIN:
					{
						if ((player.getRace() == Race.ELF) && player.isInCategory(CategoryType.FOURTH_CLASS_GROUP))
						{
							htmltext = "30856-01.htm";
						}
						else
						{
							htmltext = "noRace.htm";
						}
						break;
					}
					case OLTRAN:
					{
						if ((player.getRace() == Race.DARK_ELF) && player.isInCategory(CategoryType.FOURTH_CLASS_GROUP))
						{
							htmltext = "30862-01.htm";
						}
						else
						{
							htmltext = "noRace.htm";
						}
						break;
					}
					case LADANZA:
					{
						if ((player.getRace() == Race.ORC) && player.isInCategory(CategoryType.FOURTH_CLASS_GROUP))
						{
							htmltext = "30865-01.htm";
						}
						else
						{
							htmltext = "noRace.htm";
						}
						break;
					}
					case FERRIS:
					{
						if ((player.getRace() == Race.DWARF) && player.isInCategory(CategoryType.FOURTH_CLASS_GROUP))
						{
							htmltext = "30847-01.htm";
						}
						else
						{
							htmltext = "noRace.htm";
						}
						break;
					}
					case BROME:
					{
						if ((player.getRace() == Race.KAMAEL) && player.isInCategory(CategoryType.FOURTH_CLASS_GROUP))
						{
							htmltext = "32221-01.htm";
						}
						else
						{
							htmltext = "noRace.htm";
						}
						break;
					}
				}
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case ANDREI:
					{
						if (qs.isCond(1))
						{
							htmltext = "31292-01.html";
						}
						else if (qs.getCond() > 1)
						{
							htmltext = "31292-04.html";
						}
						break;
					}
					case GERKENSHTEIN:
					{
						if (qs.isCond(2))
						{
							htmltext = "33648-01.html";
						}
						else if (qs.getCond() == 3)
						{
							htmltext = "33648-03.html";
						}
						else if (qs.isCond(4))
						{
							htmltext = "33648-04.html";
							takeItems(player, REMNANT_ASHES, -1);
							addExpAndSp(player, EXP_REWARD, SP_REWARD);
							giveItems(player, ADENA);
							qs.exitQuest(false, true);
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				switch (npc.getId())
				{
					case ANDREI:
					{
						htmltext = "31292-05.html";
					}
						break;
					case GERKENSHTEIN:
					{
						htmltext = "33648-05.html";
					}
						break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && (qs.isCond(3)))
		{
			if (giveItemRandomly(killer, npc, REMNANT_ASHES, 1, 30, 0.15, true))
			{
				qs.setCond(4, true);
			}
		}
	}
}
