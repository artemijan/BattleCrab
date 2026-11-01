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
package quests.Q00561_BasicMissionHarnakUndergroundRuins;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowHtml;

/**
 * Q00561_BasicMissionHarnakUndergroundRuins
 * @URL http://l2on.net/en/?c=quests&id=561&game=1
 * @author NightBR
 */
public class Q00561_BasicMissionHarnakUndergroundRuins extends Quest
{
	// NPCs
	private static final int PENNY = 34413;
	private static final int ELISA = 30848;
	private static final int MILIA = 30006;
	private static final int HADEL = 33344;
	
	// Rewards
	private static final long EXP = 115930275;
	private static final int SP = 115920;
	private static final int FP = 100; // Faction points
	private static final int SCROLL_OF_ESCAPE_HARNAK_UNDERGROUND_RUINS = 39496;
	private static final int SCROLL_OF_ESCAPE_TOWN_OF_ADEN = 48413;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final int MAX_LEVEL = 87;
	
	public Q00561_BasicMissionHarnakUndergroundRuins()
	{
		super(561);
		addStartNpc(PENNY);
		addTalkId(PENNY, ELISA, MILIA, HADEL);
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
			case "34413-06.html":
			case "34413-10.html":
			case "30848-02.html":
			case "30848-03.html":
			case "30006-02.html":
			case "30006-03.html":
			{
				htmltext = event;
				break;
			}
			case "34413-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34413-05.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34413-09.html":
			{
				// Show Service/Help/Faction System page
				// TODO: Find the correct Id for player.sendPacket(new ExTutorialShowId(22));
				player.sendPacket(new TutorialShowHtml(npc.getObjectId(), "..\\L2Text\\help_faction.htm", TutorialShowHtml.LARGE_WINDOW));
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34413-07.html":
			{
				// Rewards
				giveItems(player, SCROLL_OF_ESCAPE_HARNAK_UNDERGROUND_RUINS, 1);
				addExpAndSp(player, EXP, SP);
				addFactionPoints(player, Faction.ADVENTURE_GUILD, FP); // add FP points to ADVENTURE_GUILD Faction
				qs.exitQuest(QuestType.DAILY, true);
				htmltext = event;
				break;
			}
			case "30848-04.html": // ELISA
			{
				// Show Service/Help/Gatekeeper page
				player.sendPacket(new ExTutorialShowId(55));
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "30006-04.html": // MILIA
			{
				qs.setCond(4, true);
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
						if (qs.getCond() == 1)
						{
							htmltext = "34413-04.htm";
						}
						else if (qs.getCond() == 2)
						{
							htmltext = "33509-10.htm";
						}
						else if (qs.getCond() == 4)
						{
							htmltext = "34413-05.html";
						}
						else
						{
							htmltext = "34413-06.html";
						}
						break;
					}
					case ELISA:
					{
						htmltext = "30848-01.html";
						break;
					}
					case MILIA:
					{
						htmltext = "30006-01.html";
						break;
					}
					case HADEL:
					{
						if (qs.getCond() == 4)
						{
							qs.setCond(5, true);
							htmltext = "33344-01.html";
						}
						else if (qs.getCond() == 5)
						{
							final QuestState st = player.getQuestState("Q00580_BeyondTheMemories");
							if ((st != null) && st.isCompleted())
							{
								qs.setCond(7, true);
								giveItems(player, SCROLL_OF_ESCAPE_TOWN_OF_ADEN, 1);
								htmltext = null;
							}
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					qs.setState(State.CREATED);
					htmltext = "34413-01.htm";
				}
				else
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				break;
			}
		}
		
		return htmltext;
	}
}
