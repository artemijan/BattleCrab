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
package quests.Q00781_UtilizeTheDarknessSeedOfAnnihilation;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Utilize the Darkness - Seed of Annihilation (781)
 * @author Kazumi
 */
public class Q00781_UtilizeTheDarknessSeedOfAnnihilation extends Quest
{
	// NPCs
	private static final int KLEMIS = 32734;
	private static final int[] MONSTER_LIST =
	{
		// Seed of Annihilation - Bistakon
		22746, // Bgurent
		22747, // Brakian
		22748, // Groikan
		22749, // Treykan
		22750, // Elite Bgurent
		22751, // Elite Brakian
		22752, // Elite Groikan
		22753, // Elite Treykan
		// Seed of Annihilation - Reptilikon
		22754, // Turtlelian
		22755, // Krajian
		22756, // Tardyon
		22757, // Elite Turtlelian
		22758, // Elite Krajian
		22759, // Elite Tardyon
		// Seed of Annihilation - Kokracon
		22760, // Kanibi
		22761, // Kiriona
		22762, // Kaiona
		22763, // Elite Kanibi
		22764, // Elite Kiriona
		22765, // Elite Kaiona
	
	};
	
	// Items
	private static final int SOUL_STONE_DUST = 15536;
	private static final int SOUL_STONE_FRAGMENT = 15486;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final int MIN_COUNT = 50;
	private static final int MAX_COUNT = 500;
	
	public Q00781_UtilizeTheDarknessSeedOfAnnihilation()
	{
		super(781);
		addStartNpc(KLEMIS);
		addTalkId(KLEMIS);
		addKillId(MONSTER_LIST);
		registerQuestItems(SOUL_STONE_DUST);
		addCondMinLevel(MIN_LEVEL, "clemis_q0781_02.htm");
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
			case "clemis_q0781_03.htm":
			case "clemis_q0781_09.htm":
			{
				htmltext = event;
				break;
			}
			case "clemis_q0781_04.htm":
			{
				qs.startQuest();
				break;
			}
			case "clemis_q0781_08.htm":
			{
				if (qs.isCond(2) || qs.isCond(3))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						final long itemCount = getQuestItemsCount(player, SOUL_STONE_DUST);
						takeItems(player, SOUL_STONE_DUST, itemCount);
						giveItems(player, SOUL_STONE_FRAGMENT, itemCount / 5);
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
				htmltext = "clemis_q0781_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "clemis_q0781_05.htm";
						break;
					}
					case 2:
					{
						htmltext = "clemis_q0781_06.htm";
						break;
					}
					case 3:
					{
						htmltext = "clemis_q0781_07.htm";
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
				giveItemRandomly(killer, npc, SOUL_STONE_DUST, 1, MAX_COUNT, 0.25, true);
				if ((getQuestItemsCount(killer, SOUL_STONE_DUST) >= MIN_COUNT) && qs.isCond(1))
				{
					qs.setCond(2, true);
				}
				
				if ((getQuestItemsCount(killer, SOUL_STONE_DUST) == MAX_COUNT) && qs.isCond(2))
				{
					qs.setCond(3, true);
				}
			}
		}
	}
}
