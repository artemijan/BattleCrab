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
package quests.Q00138_TempleChampionPart2;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q00137_TempleChampionPart1.Q00137_TempleChampionPart1;

/**
 * Temple Champion - 2 (138)
 * @author nonom, Gladicek
 */
public class Q00138_TempleChampionPart2 extends Quest
{
	// NPCs
	private static final int SYLVAIN = 30070;
	private static final int PUPINA = 30118;
	private static final int ANGUS = 30474;
	private static final int SLA = 30666;
	private static final int[] MOBS =
	{
		20176, // Wyrm
		20550, // Guardian Basilisk
		20551, // Road Scavenger
		20552, // Fettered Soul
	};
	
	// Items
	private static final int TEMPLE_MANIFESTO = 10341;
	private static final int RELICS_OF_THE_DARK_ELF_TRAINEE = 10342;
	private static final int ANGUS_RECOMMENDATION = 10343;
	private static final int PUPINAS_RECOMMENDATION = 10344;
	
	// Misc
	private static final int MIN_LEVEL = 36;
	private static final int MAX_LEVEL = 42;
	
	public Q00138_TempleChampionPart2()
	{
		super(138);
		addStartNpc(SYLVAIN);
		addTalkId(SYLVAIN, PUPINA, ANGUS, SLA);
		addKillId(MOBS);
		addCondMinLevel(MIN_LEVEL, "30070-10.htm");
		addCondCompletedQuest(Q00137_TempleChampionPart1.class.getSimpleName(), "30070-11.htm");
		registerQuestItems(TEMPLE_MANIFESTO, RELICS_OF_THE_DARK_ELF_TRAINEE, ANGUS_RECOMMENDATION, PUPINAS_RECOMMENDATION);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		String htmltext = event;
		
		switch (event)
		{
			case "30070-02.htm":
			{
				qs.startQuest();
				giveItems(player, TEMPLE_MANIFESTO, 1);
				break;
			}
			case "30070-05.html":
			{
				if (player.getLevel() >= MIN_LEVEL)
				{
					giveAdena(player, 84593, true);
					if ((player.getLevel() < MAX_LEVEL))
					{
						addExpAndSp(player, 187062, 20);
					}
					
					qs.exitQuest(false, true);
				}
				else
				{
					htmltext = getNoQuestLevelRewardMsg(player);
				}
				break;
			}
			case "30070-03.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2, true);
				}
				break;
			}
			case "30118-06.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
				}
				break;
			}
			case "30118-09.html":
			{
				if (qs.isCond(5))
				{
					qs.setCond(6, true);
					giveItems(player, PUPINAS_RECOMMENDATION, 1);
				}
				break;
			}
			case "30474-02.html":
			{
				if (qs.isCond(3))
				{
					qs.setCond(4, true);
				}
				break;
			}
			case "30666-02.html":
			{
				if (hasQuestItems(player, PUPINAS_RECOMMENDATION))
				{
					qs.setMemoState(1);
					takeItems(player, PUPINAS_RECOMMENDATION, -1);
				}
				break;
			}
			case "30666-03.html":
			{
				if (hasQuestItems(player, TEMPLE_MANIFESTO))
				{
					qs.setMemoState(2);
					takeItems(player, TEMPLE_MANIFESTO, -1);
				}
				break;
			}
			case "30666-08.html":
			{
				if (qs.isCond(6))
				{
					qs.setCond(7, true);
					qs.setMemoState(0);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(4) && (getQuestItemsCount(player, RELICS_OF_THE_DARK_ELF_TRAINEE) < 10))
		{
			giveItems(player, RELICS_OF_THE_DARK_ELF_TRAINEE, 1);
			if (getQuestItemsCount(player, RELICS_OF_THE_DARK_ELF_TRAINEE) >= 10)
			{
				playSound(player, QuestSound.ITEMSOUND_QUEST_MIDDLE);
			}
			else
			{
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == SYLVAIN)
				{
					htmltext = "30070-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case SYLVAIN:
					{
						switch (qs.getCond())
						{
							case 1:
								htmltext = "30070-02.htm";
								break;
							case 2:
							case 3:
							case 4:
							case 5:
							case 6:
								htmltext = "30070-03.html";
								break;
							case 7:
								htmltext = "30070-04.html";
								break;
						}
						break;
					}
					case PUPINA:
					{
						switch (qs.getCond())
						{
							case 2:
								htmltext = "30118-01.html";
								break;
							case 3:
							case 4:
								htmltext = "30118-07.html";
								break;
							case 5:
							{
								if (hasQuestItems(player, ANGUS_RECOMMENDATION))
								{
									takeItems(player, ANGUS_RECOMMENDATION, -1);
									htmltext = "30118-08.html";
								}
								break;
							}
							case 6:
								htmltext = "30118-10.html";
								break;
						}
						break;
					}
					case ANGUS:
					{
						switch (qs.getCond())
						{
							case 3:
								htmltext = "30474-01.html";
								break;
							case 4:
							{
								if (getQuestItemsCount(player, RELICS_OF_THE_DARK_ELF_TRAINEE) >= 10)
								{
									takeItems(player, RELICS_OF_THE_DARK_ELF_TRAINEE, -1);
									giveItems(player, ANGUS_RECOMMENDATION, 1);
									qs.setCond(5, true);
									htmltext = "30474-04.html";
								}
								else
								{
									htmltext = "30474-03.html";
								}
								break;
							}
							case 5:
								htmltext = "30474-05.html";
								break;
						}
						break;
					}
					case SLA:
					{
						switch (qs.getCond())
						{
							case 6:
							{
								switch (qs.getMemoState())
								{
									case 1:
										htmltext = "30666-02.html";
										break;
									case 2:
										htmltext = "30666-03.html";
										break;
									default:
										htmltext = "30666-01.html";
										break;
								}
								break;
							}
							case 7:
								htmltext = "30666-09.html";
								break;
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
}
