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
package quests.Q00782_UtilizeTheDarknessSeedOfHellfire;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Utilize the Darkness - Seed of Hellfire (782)
 * @author Kazumi
 */
public class Q00782_UtilizeTheDarknessSeedOfHellfire extends Quest
{
	// NPCs
	private static final int SIZRAK = 33669;
	private static final int[] MONSTER_LIST =
	{
		23213, // Beggar Zofan
		23214, // Beggar Zofan
		23215, // Zofan
		23216, // Zofan
		23217, // Young Zofan
		23218, // Young Zofan
		23219, // Engineer Zofan
		23220, // Kunda Watchman
		23221, // Adac the Engineer
		23222, // Borok the Engineer
		23223, // Koja the Engineer
		23224, // Kunda Guardian
		23225, // Kunda Berserker
		23226, // Kunda Executor
		23227, // Beggar Zofan
		23228, // Beggar Zofan
		23229, // Zofan
		23230, // Zofan
		23231, // Young Zofan
		23232, // Young Zofan
		23233, // Engineer Zofan
		23234, // Engineer Zofan
		23235, // Kunda
		23236, // Kunda
		23237, // Engineer Zofan
	};
	
	// Items
	private static final int DISABLED_PETRA = 34976;
	private static final int PETRA = 34959;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	private static final int MIN_COUNT = 50;
	private static final int MAX_COUNT = 500;
	
	public Q00782_UtilizeTheDarknessSeedOfHellfire()
	{
		super(782);
		addStartNpc(SIZRAK);
		addTalkId(SIZRAK);
		addKillId(MONSTER_LIST);
		registerQuestItems(DISABLED_PETRA);
		addCondMinLevel(MIN_LEVEL, "sofa_sizraku_q0782_02.htm");
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
			case "sofa_sizraku_q0782_03.htm":
			case "sofa_sizraku_q0782_09.htm":
			{
				htmltext = event;
				break;
			}
			case "sofa_sizraku_q0782_04.htm":
			{
				qs.startQuest();
				break;
			}
			case "sofa_sizraku_q0782_08.htm":
			{
				if (qs.isCond(2) || qs.isCond(3))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						final long itemCount = getQuestItemsCount(player, DISABLED_PETRA);
						takeItems(player, DISABLED_PETRA, itemCount);
						giveItems(player, PETRA, itemCount / 5);
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
				htmltext = "sofa_sizraku_q0782_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "sofa_sizraku_q0782_05.htm";
						break;
					}
					case 2:
					{
						htmltext = "sofa_sizraku_q0782_06.htm";
						break;
					}
					case 3:
					{
						htmltext = "sofa_sizraku_q0782_07.htm";
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
				giveItemRandomly(killer, npc, DISABLED_PETRA, 1, MAX_COUNT, 0.25, true);
				if ((getQuestItemsCount(killer, DISABLED_PETRA) >= MIN_COUNT) && qs.isCond(1))
				{
					qs.setCond(2, true);
				}
				
				if ((getQuestItemsCount(killer, DISABLED_PETRA) == MAX_COUNT) && qs.isCond(2))
				{
					qs.setCond(3, true);
				}
			}
		}
	}
}
