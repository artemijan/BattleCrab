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
package quests.Q00031_SecretBuriedInTheSwamp;

import java.util.Arrays;
import java.util.List;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Secret Buried in the Swamp (31)
 * @author Janiko
 */
public class Q00031_SecretBuriedInTheSwamp extends Quest
{
	// NPCs
	private static final int ABERCROMBIE = 31555;
	private static final int FORGOTTEN_MONUMENT_1 = 31661;
	private static final int FORGOTTEN_MONUMENT_2 = 31662;
	private static final int FORGOTTEN_MONUMENT_3 = 31663;
	private static final int FORGOTTEN_MONUMENT_4 = 31664;
	private static final int CORPSE_OF_DWARF = 31665;
	
	// Items
	private static final int KRORINS_JOURNAL = 7252;
	
	// Misc
	private static final int MIN_LEVEL = 66;
	
	// Monuments
	private static final List<Integer> MONUMENTS = Arrays.asList(FORGOTTEN_MONUMENT_1, FORGOTTEN_MONUMENT_2, FORGOTTEN_MONUMENT_3, FORGOTTEN_MONUMENT_4);
	
	public Q00031_SecretBuriedInTheSwamp()
	{
		super(31);
		addStartNpc(ABERCROMBIE);
		addTalkId(ABERCROMBIE, CORPSE_OF_DWARF);
		addTalkId(MONUMENTS);
		registerQuestItems(KRORINS_JOURNAL);
		addCondMinLevel(MIN_LEVEL, "31555-03.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		String htmltext = null;
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "31555-02.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "31665-02.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2, true);
					giveItems(player, KRORINS_JOURNAL, 1);
					htmltext = event;
				}
				break;
			}
			case "31555-05.html":
			{
				if (qs.isCond(2) && hasQuestItems(player, KRORINS_JOURNAL))
				{
					takeItems(player, KRORINS_JOURNAL, -1);
					qs.setCond(3, true);
					htmltext = event;
				}
				break;
			}
			case "31661-02.html":
			case "31662-02.html":
			case "31663-02.html":
			case "31664-02.html":
			{
				if (MONUMENTS.contains(npc.getId()) && qs.isCond(MONUMENTS.indexOf(npc.getId()) + 3))
				{
					qs.setCond(qs.getCond() + 1, true);
					htmltext = event;
				}
				break;
			}
			case "31555-08.html":
			{
				if (qs.isCond(7))
				{
					giveAdena(player, 343430, true);
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 1_650_970, 396);
					}
					
					qs.exitQuest(false, true);
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
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (npc.getId())
		{
			case ABERCROMBIE:
			{
				switch (qs.getState())
				{
					case State.CREATED:
					{
						htmltext = "31555-01.htm";
						break;
					}
					case State.STARTED:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "31555-02.html";
								break;
							}
							case 2:
							{
								if (hasQuestItems(player, KRORINS_JOURNAL))
								{
									htmltext = "31555-04.html";
								}
								break;
							}
							case 3:
							{
								htmltext = "31555-06.html";
								break;
							}
							case 7:
							{
								htmltext = "31555-07.html";
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
				break;
			}
			case CORPSE_OF_DWARF:
			{
				if (qs.isCond(1))
				{
					htmltext = "31665-01.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "31665-03.html";
				}
				break;
			}
			case FORGOTTEN_MONUMENT_1:
			case FORGOTTEN_MONUMENT_2:
			case FORGOTTEN_MONUMENT_3:
			case FORGOTTEN_MONUMENT_4:
			{
				final int loc = MONUMENTS.indexOf(npc.getId()) + 3;
				if (qs.isCond(loc))
				{
					htmltext = npc.getId() + "-01.html";
				}
				else if (qs.isCond(loc + 1))
				{
					htmltext = npc.getId() + "-03.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
}
