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
package quests.Q10561_AcrossTheDeathLine;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.Q10560_WayOfWanderingKnight.Q10560_WayOfWanderingKnight;

/**
 * Across The DeathLine (10561)
 * @URL https://l2wiki.com/Across_the_Death_Line
 * @author NightBR
 */
public class Q10561_AcrossTheDeathLine extends Quest
{
	// NPCs
	private static final int HERPHAH = 34362;
	private static final int PENNY = 34413;
	
	// Reward's
	private static final long EXP = 4409345453L;
	private static final int SP = 3968411;
	private static final int SOUL_SHOT_GRADE_R = 22433;
	private static final int BS_SHOT_GRADE_R = 22434;
	private static final int PA_ART_OF_SEDUCTION = 37928;
	private static final int TALISMAN_DESTRUCTION = 34985;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final int MAX_LEVEL = 97;
	
	public Q10561_AcrossTheDeathLine()
	{
		super(10561);
		addStartNpc(HERPHAH);
		addTalkId(HERPHAH, PENNY);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "nolevel.html");
		addCondCompletedQuest(Q10560_WayOfWanderingKnight.class.getSimpleName(), "34362-99.html");
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
			case "34362-02.htm":
			case "34362-03.htm":
			case "34362-05.html":
			case "34362-06.html":
			{
				htmltext = event;
				break;
			}
			case "34362-04.htm":
			{
				// show Service/Help/Death Penalty page
				player.sendPacket(new ExTutorialShowId(60));
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34362-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34362-09.html":
			{
				// Rewards
				giveItems(player, SOUL_SHOT_GRADE_R, 2500);
				giveItems(player, BS_SHOT_GRADE_R, 2500);
				giveItems(player, PA_ART_OF_SEDUCTION, 5);
				giveItems(player, TALISMAN_DESTRUCTION, 1);
				addExpAndSp(player, EXP, SP);
				qs.exitQuest(QuestType.ONE_TIME, true);
				htmltext = event;
				break;
			}
			case "34413-02.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34413-04.html":
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
				if (npc.getId() == HERPHAH)
				{
					htmltext = "34362-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case HERPHAH:
					{
						if (qs.isCond(1))
						{
							htmltext = "34362-04.htm";
						}
						else if (qs.isCond(4))
						{
							htmltext = "34362-08.html";
						}
						else
						{
							htmltext = "34362-07.html";
						}
						break;
					}
					case PENNY:
					{
						if (qs.isCond(2))
						{
							htmltext = "34413-01.html";
						}
						else if (qs.isCond(3))
						{
							addFactionPoints(player, Faction.ADVENTURE_GUILD, 200);
							
							// TODO: we need to add reward % of amity points to factions in all faction quests
							// Checking if reached level 2 with Adventurer's Guild Faction
							if (player.getFactionLevel(Faction.ADVENTURE_GUILD) >= 2)
							{
								htmltext = "34413-03.html";
							}
							else
							{
								htmltext = "noAmity.html";
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
}
