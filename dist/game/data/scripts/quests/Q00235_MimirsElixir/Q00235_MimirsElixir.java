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
package quests.Q00235_MimirsElixir;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.quest.Quest;
import org.l2jmobius.gameserver.model.quest.QuestState;
import org.l2jmobius.gameserver.model.quest.State;
import org.l2jmobius.gameserver.network.serverpackets.MagicSkillUse;
import org.l2jmobius.gameserver.network.serverpackets.SocialAction;

public class Q00235_MimirsElixir extends Quest
{
	// NPCs
	private static final int JOAN = 30718;
	private static final int LADD = 30721;
	private static final int MIXING_URN = 31149;
	// Items
	private static final int STAR_OF_DESTINY = 5011;
	private static final int PURE_SILVER = 6320;
	private static final int TRUE_GOLD = 6321;
	private static final int SAGE_STONE = 6322;
	private static final int BLOOD_FIRE = 6318;
	private static final int MIMIR_ELIXIR = 6319;
	private static final int MAGISTER_MIXING_STONE = 5905;
	// Reward
	private static final int SCROLL_ENCHANT_WEAPON_A = 729;
	
	public Q00235_MimirsElixir()
	{
		super(235);
		registerQuestItems(PURE_SILVER, TRUE_GOLD, SAGE_STONE, BLOOD_FIRE, MAGISTER_MIXING_STONE, MIMIR_ELIXIR);
		addStartNpc(LADD);
		addTalkId(LADD, JOAN, MIXING_URN);
		addKillId(20965, 21090);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		final QuestState st = getQuestState(player, false);
		if (st == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "30721-06.htm":
			{
				st.startQuest();
				break;
			}
			case "30721-12.htm":
			{
				if (hasQuestItems(player, TRUE_GOLD))
				{
					st.setCond(6, true);
					giveItems(player, MAGISTER_MIXING_STONE, 1);
				}
				break;
			}
			case "30721-16.htm":
			{
				if (hasQuestItems(player, MIMIR_ELIXIR))
				{
					player.broadcastPacket(new MagicSkillUse(player, player, 4339, 1, 1, 1));
					takeItems(player, MAGISTER_MIXING_STONE, -1);
					takeItems(player, MIMIR_ELIXIR, -1);
					takeItems(player, STAR_OF_DESTINY, -1);
					giveItems(player, SCROLL_ENCHANT_WEAPON_A, 1);
					player.broadcastPacket(new SocialAction(player.getObjectId(), 3));
					st.exitQuest(false, true);
				}
				break;
			}
			case "30718-03.htm":
			{
				st.setCond(3, true);
				break;
			}
			case "31149-02.htm":
			{
				if (!hasQuestItems(player, MAGISTER_MIXING_STONE))
				{
					htmltext = "31149-havent.htm";
				}
				break;
			}
			case "31149-03.htm":
			{
				if (!hasQuestItems(player, MAGISTER_MIXING_STONE, PURE_SILVER))
				{
					htmltext = "31149-havent.htm";
				}
				break;
			}
			case "31149-05.htm":
			{
				if (!hasQuestItems(player, MAGISTER_MIXING_STONE, PURE_SILVER, TRUE_GOLD))
				{
					htmltext = "31149-havent.htm";
				}
				break;
			}
			case "31149-07.htm":
			{
				if (!hasQuestItems(player, MAGISTER_MIXING_STONE, PURE_SILVER, TRUE_GOLD, BLOOD_FIRE))
				{
					htmltext = "31149-havent.htm";
				}
				break;
			}
			case "31149-success.htm":
			{
				if (hasQuestItems(player, MAGISTER_MIXING_STONE, PURE_SILVER, TRUE_GOLD, BLOOD_FIRE))
				{
					st.setCond(8, true);
					takeItems(player, PURE_SILVER, -1);
					takeItems(player, TRUE_GOLD, -1);
					takeItems(player, BLOOD_FIRE, -1);
					giveItems(player, MIMIR_ELIXIR, 1);
				}
				else
				{
					htmltext = "31149-havent.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState st = getQuestState(player, true);
		
		switch (st.getState())
		{
			case State.CREATED:
			{
				if (player.getLevel() < 75)
				{
					htmltext = "30721-01b.htm";
				}
				else if (!hasQuestItems(player, STAR_OF_DESTINY))
				{
					htmltext = "30721-01a.htm";
				}
				else
				{
					htmltext = "30721-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				final int cond = st.getCond();
				switch (npc.getId())
				{
					case LADD:
					{
						if (cond == 1)
						{
							if (hasQuestItems(player, PURE_SILVER))
							{
								htmltext = "30721-08.htm";
								st.setCond(2, true);
							}
							else
							{
								htmltext = "30721-07.htm";
							}
						}
						else if (cond < 5)
						{
							htmltext = "30721-10.htm";
						}
						else if ((cond == 5) && hasQuestItems(player, TRUE_GOLD))
						{
							htmltext = "30721-11.htm";
						}
						else if ((cond == 6) || (cond == 7))
						{
							htmltext = "30721-13.htm";
						}
						else if ((cond == 8) && hasQuestItems(player, MIMIR_ELIXIR))
						{
							htmltext = "30721-14.htm";
						}
						break;
					}
					case JOAN:
					{
						if (cond == 2)
						{
							htmltext = "30718-01.htm";
						}
						else if (cond == 3)
						{
							htmltext = "30718-04.htm";
						}
						else if ((cond == 4) && hasQuestItems(player, SAGE_STONE))
						{
							htmltext = "30718-05.htm";
							st.setCond(5, true);
							takeItems(player, SAGE_STONE, -1);
							giveItems(player, TRUE_GOLD, 1);
						}
						else if (cond > 4)
						{
							htmltext = "30718-06.htm";
						}
						break;
					}
					// The urn gives the same first htm. Bypasses' events will do all the job.
					case MIXING_URN:
					{
						htmltext = "31149-01.htm";
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
	public String onKill(Npc npc, Player player, boolean isPet)
	{
		final QuestState st = getQuestState(player, false);
		if ((st == null) || !st.isStarted())
		{
			return null;
		}
		
		switch (npc.getId())
		{
			case 20965:
			{
				if (st.isCond(3) && (getRandom(10) < 2))
				{
					giveItems(player, SAGE_STONE, 1);
					st.setCond(4, true);
				}
				break;
			}
			case 21090:
			{
				if (st.isCond(6) && (getRandom(10) < 2))
				{
					giveItems(player, BLOOD_FIRE, 1);
					st.setCond(7, true);
				}
				break;
			}
		}
		
		return null;
	}
}
