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
package quests.Q00564_BasicMissionKartiasLabyrinthSolo;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

/**
 * Q00564_KartiasLabyrinthSolo
 * @URL https://l2wiki.com/Basic_Mission:_Kartia%27s_Labyrinth_(Solo)
 * @author NightBR
 */
public class Q00564_BasicMissionKartiasLabyrinthSolo extends Quest
{
	// NPCs
	private static final int PENNY = 34413;
	private static final int KARTIA = 33647;
	
	// Reward's
	private static final long EXP = 1409345453;
	private static final int SP = 3968411;
	private static final int SCROLL_OF_ESCAPE_KARTIAS_LABYRINTH = 39497;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final int MAX_LEVEL = 97;
	
	public Q00564_BasicMissionKartiasLabyrinthSolo()
	{
		super(564);
		addStartNpc(PENNY);
		addTalkId(PENNY, KARTIA);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "nolevel.html");
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
			case "34413-02.htm":
			case "34413-03.htm":
			case "34413-05.html":
			{
				htmltext = event;
				break;
			}
			case "34413-04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34413-06.html":
			{
				// Show Service/Help/Instance Zone page
				player.sendPacket(new ExTutorialShowId(29));
				htmltext = event;
				break;
			}
			case "34413-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34413-09.html":
			{
				final StringBuilder str = new StringBuilder("000");
				checkQuestCompleted(player, str); // Initialize the array with all quests completed
				if (str.indexOf("1") == -1) // verify if no quest completed
				{
					htmltext = "34413-07.html";
				}
				else
				{
					if (str.charAt(0) == '1')
					{
						addFactionPoints(player, Faction.ADVENTURE_GUILD, 100); // add 100 points to ADVENTURE_GUILD Faction
					}
					
					if (str.charAt(1) == '1')
					{
						addFactionPoints(player, Faction.ADVENTURE_GUILD, 125); // add 125 points to ADVENTURE_GUILD Faction
					}
					
					if (str.charAt(2) == '1')
					{
						addFactionPoints(player, Faction.ADVENTURE_GUILD, 150); // add 150 points to ADVENTURE_GUILD Faction
					}
					
					giveItems(player, SCROLL_OF_ESCAPE_KARTIAS_LABYRINTH, 1);
					addExpAndSp(player, EXP, SP);
					qs.exitQuest(QuestType.DAILY, true);
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
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == PENNY)
				{
					htmltext = "34413-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case PENNY:
					{
						htmltext = qs.isCond(2) ? "34413-07.html" : "34413-08.html";
						break;
					}
					case KARTIA:
					{
						if (qs.isCond(2))
						{
							final StringBuilder str = new StringBuilder("000");
							checkQuestCompleted(player, str); // Initialize the array with all quests completed
							if (str.indexOf("1") != -1) // verify if any quest completed
							{
								qs.setCond(4, true);
								htmltext = "33647-02.html";
							}
							else
							{
								htmltext = "33647-01.html";
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
	
	private StringBuilder checkQuestCompleted(Player player, StringBuilder string)
	{
		int index = 0;
		final char ch = '1';
		final QuestState st1 = player.getQuestState("Q00497_IncarnationOfGreedZellakaSolo");
		if ((st1 != null) && st1.isCompleted())
		{
			index = 0;
			string.setCharAt(index, ch);
		}
		
		final QuestState st2 = player.getQuestState("Q00498_IncarnationOfJealousyPellineSolo");
		if ((st2 != null) && st2.isCompleted())
		{
			index = 1;
			string.setCharAt(index, ch);
		}
		
		final QuestState st3 = player.getQuestState("Q00499_IncarnationOfGluttonyKaliosSolo");
		if ((st3 != null) && st3.isCompleted())
		{
			index = 2;
			string.setCharAt(index, ch);
		}
		
		return string;
	}
}
