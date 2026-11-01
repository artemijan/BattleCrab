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
package quests.Q00747_DefendingTheForsakenPlains;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Defending the Forsaken Plains (747)
 * @author Kazumi
 */
public final class Q00747_DefendingTheForsakenPlains extends Quest
{
	// NPCs
	private static final int PATERSON = 33864;
	private static final int EBLUNE = 33865;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		20679, // Marsh Stalker
		20680, // Marsh Drake
		21017, // Fallen Orc
		21018, // Ancient Gargoyle
		21019, // Fallen Orc Archer
		21020, // Fallen Orc Shaman
		21021, // Sharp Talon Tiger
		21022, // Fallen Orc Captain
		20647, // Yintzu
		20648, // Paliote
		20649, // Hamrut
		20650, // Kranrot
	};
	
	// Items
	private static final int MARK_OF_THE_PLAINS = 47051;
	private static final int MARK_OF_THE_PLATEAU = 47052;
	private static final int PAULINAS_EQUIPMENT_SET_A = 46851;
	
	// Misc
	private static final int MIN_LEVEL = 56;
	private static final int MAX_LEVEL = 60;
	
	public Q00747_DefendingTheForsakenPlains()
	{
		super(747);
		addStartNpc(PATERSON);
		addTalkId(PATERSON, EBLUNE);
		addKillId(MONSTERS);
		registerQuestItems(MARK_OF_THE_PLAINS, MARK_OF_THE_PLATEAU);
		addCondMinLevel(MIN_LEVEL, "petterzan_q0747_02.htm");
		addCondMaxLevel(MAX_LEVEL, "petterzan_q0747_02.htm");
		addCondIsNotSubClassActive("petterzan_q0747_03.htm");
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
			case "petterzan_q0747_04.htm":
			case "petterzan_q0747_05.htm":
			case "evluena_q0747_02.htm":
			{
				htmltext = event;
				break;
			}
			case "petterzan_q0747_06.htm":
			{
				qs.startQuest();
				break;
			}
			case "evluena_q0747_03.htm":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2);
				}
				break;
			}
			case "petterzan_q0747_10.htm":
			{
				if (qs.isCond(3))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						qs.exitQuest(QuestType.REPEATABLE, true);
						addExpAndSp(player, 37709985, 4701);
						giveItems(player, PAULINAS_EQUIPMENT_SET_A, 1);
						htmltext = event;
						break;
					}
					
					htmltext = getNoQuestLevelRewardMsg(player);
					break;
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
				if (npc.getId() == PATERSON)
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						htmltext = "petterzan_q0747_01.htm";
						break;
					}
					
					htmltext = "petterzan_q0747_02.htm";
					break;
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case EBLUNE:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "evluena_q0747_01.htm";
								break;
							}
							case 2:
							{
								htmltext = "evluena_q0747_04.htm";
								break;
							}
							case 3:
							{
								htmltext = "evluena_q0747_05.htm";
								break;
							}
						}
						break;
					}
					case PATERSON:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "petterzan_q0747_07.htm";
								break;
							}
							case 2:
							{
								htmltext = "petterzan_q0747_08.htm";
								break;
							}
							case 3:
							{
								htmltext = "petterzan_q0747_09.htm";
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
				qs.setState(State.CREATED);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, true);
		if ((qs != null) && qs.isCond(2))
		{
			switch (npc.getId())
			{
				case 20679: // Marsh Stalker
				case 20680: // Marsh Drake
				case 21017: // Fallen Orc
				case 21018: // Ancient Gargoyle
				case 21019: // Fallen Orc Archer
				case 21020: // Fallen Orc Shaman
				case 21021: // Sharp Talon Tiger
				case 21022: // Fallen Orc Captain
				{
					if (giveItemRandomly(killer, npc, MARK_OF_THE_PLAINS, 1, 200, 1.0, true))
					{
						if ((getQuestItemsCount(killer, MARK_OF_THE_PLAINS) >= 200) && (getQuestItemsCount(killer, MARK_OF_THE_PLATEAU) >= 200))
						{
							qs.setCond(3, true);
						}
					}
					break;
				}
				case 20647: // Yintzu
				case 20648: // Paliote
				case 20649: // Hamrut
				case 20650: // Kranrot
				{
					if (giveItemRandomly(killer, npc, MARK_OF_THE_PLATEAU, 1, 200, 1.0, true))
					{
						if ((getQuestItemsCount(killer, MARK_OF_THE_PLATEAU) >= 200) && (getQuestItemsCount(killer, MARK_OF_THE_PLAINS) >= 200))
						{
							qs.setCond(3, true);
						}
					}
					break;
				}
			}
		}
	}
}
