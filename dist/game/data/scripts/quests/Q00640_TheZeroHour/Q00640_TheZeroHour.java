/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package quests.Q00640_TheZeroHour;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.enums.QuestSound;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.quest.Quest;
import org.l2jmobius.gameserver.model.quest.QuestState;
import org.l2jmobius.gameserver.model.quest.State;

import quests.Q00109_InSearchOfTheNest.Q00109_InSearchOfTheNest;

/**
 * The Zero Hour (640)<br>
 * @author Sacrifice
 */
public class Q00640_TheZeroHour extends Quest
{
	// NPCs
	private static final int KAHMAN = 31554;
	private static final int[] MONSTERS_TO_HUNT =
	{
		22617, // Spiked Stakato
		22618, // Spiked Stakato Worker
		22619, // Spiked Stakato Guard
		22620, // Female Spiked Stakato
		22621, // Male Spiked Stakato
		22622, // Male Spiked Stakato
		22623, // Spiked Stakato Sorcerer
		22625, // Cannibalistic Stakato Leader
		22626, // Cannibalistic Stakato Leader
		22627, // Spiked Stakato Soldier
		22628, // Spiked Stakato Drone
		22629, // Spiked Stakato Captain
		22630, // Spike Stakato Nurse
		22631, // Spike Stakato Nurse
		22633 // Spiked Stakato Shaman
	};
	// Item
	private static final int FANG_OF_STAKATO = 8085;
	// Misc
	private static final int MIN_LEVEL = 66;
	
	public Q00640_TheZeroHour()
	{
		super(640);
		addStartNpc(KAHMAN);
		addTalkId(KAHMAN);
		addKillId(MONSTERS_TO_HUNT);
		registerQuestItems(FANG_OF_STAKATO);
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
			case "31554-02.htm":
			{
				qs.startQuest();
				break;
			}
			case "31554-05.htm":
			{
				htmltext = "31554-05.htm";
				break;
			}
			case "31554-08.htm":
			{
				qs.exitQuest(true, true);
				break;
			}
			case "1":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 12)
				{
					takeItems(player, FANG_OF_STAKATO, 12);
					rewardItems(player, 4042, 1); // Enria
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
			case "2":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 6)
				{
					takeItems(player, FANG_OF_STAKATO, 6);
					rewardItems(player, 4043, 1); // Asofe
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
			case "3":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 6)
				{
					takeItems(player, FANG_OF_STAKATO, 6);
					rewardItems(player, 4044, 1); // Thons
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
			case "4":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 81)
				{
					takeItems(player, FANG_OF_STAKATO, 81);
					rewardItems(player, 1887, 10); // Varnish of Purity
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
			case "5":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 33)
				{
					takeItems(player, FANG_OF_STAKATO, 33);
					rewardItems(player, 1888, 5); // Synthetic Cokes
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
			case "6":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 30)
				{
					takeItems(player, FANG_OF_STAKATO, 30);
					rewardItems(player, 1889, 10); // Compound Braid
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
			case "7":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 150)
				{
					takeItems(player, FANG_OF_STAKATO, 150);
					rewardItems(player, 5550, 10); // Durable Metal Plate
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
			case "8":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 131)
				{
					takeItems(player, FANG_OF_STAKATO, 131);
					rewardItems(player, 1890, 10); // Mithril Alloy
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
			case "9":
			{
				if (getQuestItemsCount(player, FANG_OF_STAKATO) >= 123)
				{
					takeItems(player, FANG_OF_STAKATO, 123);
					rewardItems(player, 1893, 5); // Oriharukon
					htmltext = "31554-09.htm";
				}
				else
				{
					htmltext = "31554-07.htm";
				}
				break;
			}
		}
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		final QuestState qs = getQuestState(talker, true);
		String htmlText = getNoQuestMsg(talker);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (talker.getLevel() >= MIN_LEVEL)
				{
					final QuestState qs2 = qs.getPlayer().getQuestState(Q00109_InSearchOfTheNest.class.getSimpleName());
					if ((qs2 != null) && (qs2.getState() == State.COMPLETED))
					{
						htmlText = "31554-01.htm";
					}
					else
					{
						htmlText = "31554-10.htm";
					}
				}
				else
				{
					htmlText = "31554-00.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					if (getQuestItemsCount(talker, FANG_OF_STAKATO) >= 1)
					{
						htmlText = "31554-04.htm";
					}
					else
					{
						htmlText = "31554-03.htm";
					}
				}
				break;
			}
		}
		return htmlText;
	}
	
	@Override
	public String onKill(Npc npc, Player killer, boolean isSummon)
	{
		final Player partyMember = getRandomPartyMemberState(killer, State.STARTED);
		if (partyMember == null)
		{
			return super.onKill(npc, killer, isSummon);
		}
		
		if (partyMember.getQuestState(Q00640_TheZeroHour.class.getSimpleName()) == null)
		{
			return super.onKill(npc, killer, isSummon);
		}
		
		giveItems(partyMember, FANG_OF_STAKATO, (long) Config.RATE_QUEST_DROP);
		playSound(partyMember, QuestSound.ITEMSOUND_QUEST_ITEMGET);
		return super.onKill(npc, killer, isSummon);
	}
}
