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
package quests.Q00780_UtilizeTheDarknessSeedOfInfinity;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Utilize the Darkness - Seed of Infinity (780)
 * @author Kazumi
 */
public class Q00780_UtilizeTheDarknessSeedOfInfinity extends Quest
{
	// NPCs
	private static final int TEPIOS = 32530;
	private static final int[] MONSTER_LIST =
	{
		23405, // Suffering Zealot
		23406, // Suffering Mutant
		23407, // Suffering Hacker
		23409, // Erosion Herald
		23411, // Erosion Mutant
		23412, // Erosion Hacker
		23413, // Erosion Ark
	};
	
	// Items
	private static final int MARRED_SOUL_CRYSTAL = 38580;
	private static final int FREED_SOUL_CRYSTAL = 38576;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	private static final int MIN_COUNT = 50;
	private static final int MAX_COUNT = 500;
	
	public Q00780_UtilizeTheDarknessSeedOfInfinity()
	{
		super(780);
		addStartNpc(TEPIOS);
		addTalkId(TEPIOS);
		addKillId(MONSTER_LIST);
		registerQuestItems(MARRED_SOUL_CRYSTAL);
		addCondMinLevel(MIN_LEVEL, "officer_tepios_q0780_02.htm");
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
			case "officer_tepios_q0780_03.htm":
			case "officer_tepios_q0780_09.htm":
			{
				htmltext = event;
				break;
			}
			case "officer_tepios_q0780_04.htm":
			{
				qs.startQuest();
				break;
			}
			case "officer_tepios_q0780_08.htm":
			{
				if (qs.isCond(2) || qs.isCond(3))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						final long itemCount = getQuestItemsCount(player, MARRED_SOUL_CRYSTAL);
						takeItems(player, MARRED_SOUL_CRYSTAL, itemCount);
						giveItems(player, FREED_SOUL_CRYSTAL, itemCount / 5);
						addExpAndSp(player, 1637472704, 0);
						qs.exitQuest(true, true);
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
				htmltext = "officer_tepios_q0780_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "officer_tepios_q0780_05.htm";
						break;
					}
					case 2:
					{
						htmltext = "officer_tepios_q0780_06.htm";
						break;
					}
					case 3:
					{
						htmltext = "officer_tepios_q0780_07.htm";
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
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final Party party = killer.getParty();
		if (party != null)
		{
			party.getMembers().forEach(p -> onKill(npc, p));
		}
		else
		{
			onKill(npc, killer);
		}
	}
	
	public void onKill(Npc npc, Player killer)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && (npc.calculateDistance3D(killer) <= 1000))
		{
			if (qs.isCond(1) || qs.isCond(2))
			{
				giveItemRandomly(killer, npc, MARRED_SOUL_CRYSTAL, 1, MAX_COUNT, 0.25, true);
				if ((getQuestItemsCount(killer, MARRED_SOUL_CRYSTAL) >= MIN_COUNT) && qs.isCond(1))
				{
					qs.setCond(2, true);
				}
				
				if ((getQuestItemsCount(killer, MARRED_SOUL_CRYSTAL) == MAX_COUNT) && qs.isCond(2))
				{
					qs.setCond(3, true);
				}
			}
		}
	}
}
