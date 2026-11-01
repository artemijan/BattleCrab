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
package quests.Q00748_EndlessRevenge;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Endless Revenge (748)
 * @author Kazumi
 */
public final class Q00748_EndlessRevenge extends Quest
{
	// NPCs
	private static final int PATERSON = 33864;
	private static final int SHUVANN = 33867;
	private static final int MATHIAS = 31340;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		20974, // Spiteful Soul Leader
		20975, // Spiteful Soul Wizard
		20976, // Spiteful Soul Warrior
		21001, // Archer of Destruction
		21002, // Doom Scout
		21003, // Graveyard Lich
		21004, // Dismal Pole
		21005, // Graveyard Predator
		21006, // Doom Servant
		21007, // Doom Guard
		21008, // Doom Archer
		21009, // Doom Trooper
		21010, // Doom Warrior
		20674, // Doom Knight
	};
	
	// Items
	private static final int SLAUGHTERER_MARK = 47053;
	
	// Misc
	private static final int MIN_LEVEL = 61;
	private static final int MAX_LEVEL = 64;
	
	public Q00748_EndlessRevenge()
	{
		super(748);
		addStartNpc(PATERSON);
		addTalkId(PATERSON, SHUVANN, MATHIAS);
		addKillId(MONSTERS);
		registerQuestItems(SLAUGHTERER_MARK);
		addCondMinLevel(MIN_LEVEL, "petterzan_q0748_02.htm");
		addCondMaxLevel(MAX_LEVEL, "petterzan_q0748_02.htm");
		addCondIsNotSubClassActive("petterzan_q0748_03.htm");
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
			case "petterzan_q0748_04.htm":
			case "petterzan_q0748_05.htm":
			case "schwann_q0748_02.htm":
			{
				htmltext = event;
				break;
			}
			case "petterzan_q0748_06.htm":
			{
				qs.startQuest();
				break;
			}
			case "schwann_q0748_03.htm":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2);
				}
				break;
			}
			case "captain_mathias_q0748_03.htm":
			{
				if (qs.isCond(3))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						qs.exitQuest(QuestType.REPEATABLE, true);
						addExpAndSp(player, 49177227, 3193);
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
						htmltext = "petterzan_q0748_01.htm";
						break;
					}
					
					htmltext = "petterzan_q0748_02.htm";
					break;
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case PATERSON:
					{
						htmltext = "petterzan_q0748_07.htm";
						break;
					}
					case SHUVANN:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "schwann_q0748_01.htm";
								break;
							}
							case 2:
							{
								htmltext = "schwann_q0748_04.htm";
								break;
							}
							case 3:
							{
								htmltext = "schwann_q0748_05.htm";
								break;
							}
						}
						break;
					}
					case MATHIAS:
					{
						switch (qs.getCond())
						{
							case 1:
							case 2:
							{
								htmltext = "captain_mathias_q0748_01.htm";
								break;
							}
							case 3:
							{
								htmltext = "captain_mathias_q0748_02.htm";
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
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(2) && giveItemRandomly(killer, npc, SLAUGHTERER_MARK, 1, 200, 1.0, true))
		{
			if (getQuestItemsCount(killer, SLAUGHTERER_MARK) >= 200)
			{
				qs.setCond(3, true);
			}
		}
	}
}
