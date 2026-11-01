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
package quests.Q10353_CertificationOfValue;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Certification of Value (10353)
 * @URL https://l2wiki.com/Certification_of_Value
 * @author Gigi
 */
public class Q10353_CertificationOfValue extends Quest
{
	// NPCs
	private static final int RIEYI = 33155;
	private static final int KYUORI = 33358;
	private static final int DUMMY_MONSTER = 33349;
	private static final int[] MONSTERS =
	{
		23044, // Exhausted Vengeful Spirit
		23045, // Wings of Viciousness
		23046, // Byron's Knight
		23047, // Terestian
		23048, // Drill Demon
		23049, // Knight's Vengeful Spirit
		23050, // Escort Knight's Vengeful Spirit
		23051, // Byron's Confidant
		23052, // Evil Shadow
		23053, // Brutal Warrior
		23054, // Demon Knight
		23055, // Demon Priest
		23056, // Vampire Queen
		23057, // Bonehead Slave
		23058, // Platinum Tribe Soldier
		23059, // Platinum Tribe Archer
		23060, // Platinum Tribe Warrior
		23061, // Platinum Tribe Shaman
		23062, // Platinum Tribe Prefect
		23063, // Heaven's Palace Knight
		23064, // Heaven's Palace Fighter
		23065, // Heaven's Palace Terminator
		23066, // Heaven's Palace Priest
		23067, // Heaven's Palace Judge
		23068, // Heaven's Palace Sniper
		23101, // Slaughter Bathin
		23102, // Bathin's Knight
		23103, // Bathin's Wizard
		23104, // Elmoreden's Lady
		23105, // Elmoreden's Archer
		23106, // Elmoreden's Maid
		23107, // Elmoreden's Warrior
		23108, // Binder
		23109, // Bound Warrior
		23110, // Bound Archer
		23111, // Heaven's Palace Seraphim
		23112 // Heaven's Palace Seraphim Escort
	};
	
	// Item
	private static final int TOKEN_OF_INSOLENCE_TOWER = 17624;
	
	// Misc
	private static final int MIN_LEVEL = 48;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q10353_CertificationOfValue()
	{
		super(10353);
		addStartNpc(RIEYI);
		addTalkId(RIEYI, KYUORI);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "33155-00.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "33155-02.htm":
			case "33155-03.htm":
			case "33155-04.htm":
			case "33155-07.html":
			case "33155-08.html":
			case "33155-09.html":
			case "33358-02.html":
			{
				htmltext = event;
				break;
			}
			case "33155-05.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33358-03.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "33358-06.html":
			{
				addExpAndSp(player, 3000000, 720);
				giveItems(player, TOKEN_OF_INSOLENCE_TOWER, 1);
				qs.exitQuest(false, true);
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
				if (npc.getId() == RIEYI)
				{
					htmltext = "33155-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case RIEYI:
					{
						if (qs.isCond(1))
						{
							htmltext = "33155-06.html";
						}
						break;
					}
					case KYUORI:
					{
						if (qs.isCond(1))
						{
							htmltext = "33358-01.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "33358-04.html";
						}
						else if (qs.isCond(3))
						{
							htmltext = "33358-05.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				switch (npc.getId())
				{
					case RIEYI:
					{
						htmltext = "33155-00a.html";
						break;
					}
					case KYUORI:
					{
						htmltext = "33358-00.html";
						break;
					}
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isStarted() && qs.isCond(2))
		{
			final int killCount = qs.getInt(KILL_COUNT_VAR) + 1;
			qs.set(KILL_COUNT_VAR, killCount);
			if (killCount >= 10)
			{
				qs.setCond(0);
				qs.setCond(3, true);
			}
			else
			{
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isStarted() && qs.isCond(2))
		{
			final int killCount = qs.getInt(KILL_COUNT_VAR);
			if (killCount > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>(1);
				holder.add(new NpcLogListHolder(DUMMY_MONSTER, false, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
