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
package quests.Q00843_GiantEvolutionControl;

import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

import quests.not_done.Q00837_RequestFromTheGiantTrackers;

/**
 * Giant Evolution Control (843)
 * @author Kazumi
 */
public final class Q00843_GiantEvolutionControl extends Quest
{
	// NPCs
	private static final int KRENAHT = 34237;
	
	// Monster
	private static final int[] MONSTERS =
	{
		23791, // Bathus - Marred
		23792, // Carcass - Marred
		23793, // Kshana - Marred
		23794, // Lucas - Marred
	};
	
	// Items
	private static final int SHINE_STONE = 47171;
	private static final int SUPPLY_BOX_BASIC = 47184;
	private static final int SUPPLY_BOX_INTERMEDIATE = 47360;
	private static final int SUPPLY_BOX_ADVANCED = 47361;
	private static final int FACTION_AMITY_TOKEN = 48030;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	private static final int COUNT_BASIC = 50;
	private static final int COUNT_INTERMEDIATE = 100;
	private static final int REWARD_BASIC = 100;
	private static final int REWARD_INTERMEDIATE = 200;
	private static final int QUEST_ID = 843;
	
	public Q00843_GiantEvolutionControl()
	{
		super(843);
		addStartNpc(KRENAHT);
		addTalkId(KRENAHT);
		addKillId(MONSTERS);
		registerQuestItems(SHINE_STONE);
		
		// addCondCompletedQuest(Q10537_KamaelDisarray.class.getSimpleName(), "giantchaser_officer_q0843_02.htm"); //TODO
		addFactionLevel(Faction.GIANT_TRACKERS, 2, "giantchaser_officer_q0843_02a.htm");
		addCondMinLevel(MIN_LEVEL, "giantchaser_officer_q0843_02a.htm");
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
			case "giantchaser_officer_q0843_07.htm":
			{
				htmltext = event;
				break;
			}
			case "quest_accept":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 2) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 5))
				{
					htmltext = "giantchaser_officer_q0843_05.htm";
				}
				else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 5)
				{
					htmltext = "giantchaser_officer_q0843_05a.htm";
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
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = getAlreadyCompletedMsg(player);
					break;
				}
				
				qs.setState(State.CREATED);
				// fallthrough
			}
			case State.CREATED:
			{
				htmltext = "giantchaser_officer_q0843_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 2) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 5))
						{
							htmltext = "giantchaser_officer_q0843_08.htm";
						}
						else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 5)
						{
							htmltext = "giantchaser_officer_q0843_08.htm";
						}
						break;
					}
					case 2:
					{
						htmltext = "giantchaser_officer_q0843_11.htm";
						break;
					}
					case 3:
					{
						htmltext = "giantchaser_officer_q0843_11a.htm";
						break;
					}
					case 4:
					{
						htmltext = "giantchaser_officer_q0843_12.htm";
						break;
					}
					case 5:
					{
						htmltext = "giantchaser_officer_q0843_12a.htm";
						break;
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(KRENAHT)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 843)
		{
			switch (reply)
			{
				case 1:
				{
					showHtmlFile(player, "giantchaser_officer_q0843_03.htm", npc);
					break;
				}
				case 2:
				{
					showHtmlFile(player, "giantchaser_officer_q0843_04.htm", npc);
					break;
				}
				case 10:
				{
					if ((player.getFactionLevel(Faction.GIANT_TRACKERS) >= 2) && (player.getFactionLevel(Faction.GIANT_TRACKERS) < 5))
					{
						showHtmlFile(player, "giantchaser_officer_q0843_06.htm", npc);
					}
					else if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 5)
					{
						showHtmlFile(player, "giantchaser_officer_q0843_06a.htm", npc);
					}
					break;
				}
				case 11:
				{
					showHtmlFile(player, "giantchaser_officer_q0843_09.htm", npc);
					break;
				}
				case 12:
				{
					showHtmlFile(player, "giantchaser_officer_q0843_09a.htm", npc);
					break;
				}
				case 21:
				{
					qs.setCond(2);
					showHtmlFile(player, "giantchaser_officer_q0843_10.htm", npc);
					break;
				}
				case 22:
				{
					qs.setCond(3);
					showHtmlFile(player, "giantchaser_officer_q0843_10a.htm", npc);
					break;
				}
				case 3101:
				{
					if (qs.getCond() == 4)
					{
						qs.exitQuest(QuestType.DAILY, true);
						
						final Quest qs837 = ScriptManager.getInstance().getScript(Q00837_RequestFromTheGiantTrackers.class.getSimpleName());
						if (qs837 != null)
						{
							qs837.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
						}
						
						if (Rnd.get(100) <= 60)
						{
							giveItems(player, SUPPLY_BOX_BASIC, 1);
						}
						else
						{
							if (Rnd.get(100) <= 70)
							{
								giveItems(player, SUPPLY_BOX_INTERMEDIATE, 1);
							}
							else
							{
								giveItems(player, SUPPLY_BOX_ADVANCED, 1);
							}
						}
						
						addExpAndSp(player, 5536944000L, 13288590);
						addFactionPoints(player, Faction.GIANT_TRACKERS, REWARD_BASIC);
						showHtmlFile(player, "giantchaser_officer_q0843_13.htm", npc);
					}
					break;
				}
				case 3102:
				{
					if (qs.getCond() == 4)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							qs.exitQuest(QuestType.DAILY, true);
							
							final Quest qs837 = ScriptManager.getInstance().getScript(Q00837_RequestFromTheGiantTrackers.class.getSimpleName());
							if (qs837 != null)
							{
								qs837.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
							}
							
							takeItems(player, FACTION_AMITY_TOKEN, 1);
							if (Rnd.get(100) <= 60)
							{
								giveItems(player, SUPPLY_BOX_BASIC, 1);
							}
							else
							{
								if (Rnd.get(100) <= 70)
								{
									giveItems(player, SUPPLY_BOX_INTERMEDIATE, 1);
								}
								else
								{
									giveItems(player, SUPPLY_BOX_ADVANCED, 1);
								}
							}
							
							addExpAndSp(player, 5536944000L * 2, 13288590 * 2);
							addFactionPoints(player, Faction.GIANT_TRACKERS, REWARD_BASIC * 2);
							showHtmlFile(player, "giantchaser_officer_q0843_13.htm", npc);
							break;
						}
						
						showHtmlFile(player, "giantchaser_officer_q0843_14.htm", npc);
					}
					break;
				}
				case 3201:
				{
					if (qs.getCond() == 5)
					{
						qs.exitQuest(QuestType.DAILY, true);
						
						final Quest qs837 = ScriptManager.getInstance().getScript(Q00837_RequestFromTheGiantTrackers.class.getSimpleName());
						if (qs837 != null)
						{
							qs837.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
						}
						
						if (Rnd.get(100) <= 60)
						{
							giveItems(player, SUPPLY_BOX_BASIC, 1);
						}
						else
						{
							if (Rnd.get(100) <= 70)
							{
								giveItems(player, SUPPLY_BOX_INTERMEDIATE, 1);
							}
							else
							{
								giveItems(player, SUPPLY_BOX_ADVANCED, 1);
							}
						}
						
						addExpAndSp(player, 11073888000L, 26577180);
						addFactionPoints(player, Faction.GIANT_TRACKERS, REWARD_INTERMEDIATE);
						showHtmlFile(player, "giantchaser_officer_q0843_13a.htm", npc);
					}
					break;
				}
				case 3202:
				{
					if (qs.getCond() == 5)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							qs.exitQuest(QuestType.DAILY, true);
							
							final Quest qs837 = ScriptManager.getInstance().getScript(Q00837_RequestFromTheGiantTrackers.class.getSimpleName());
							if (qs837 != null)
							{
								qs837.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
							}
							
							takeItems(player, FACTION_AMITY_TOKEN, 1);
							if (Rnd.get(100) <= 60)
							{
								giveItems(player, SUPPLY_BOX_INTERMEDIATE, 1);
							}
							else
							{
								if (Rnd.get(100) <= 70)
								{
									giveItems(player, SUPPLY_BOX_BASIC, 1);
								}
								else
								{
									giveItems(player, SUPPLY_BOX_ADVANCED, 1);
								}
							}
							
							addExpAndSp(player, 11073888000L * 2, 26577180 * 2);
							addFactionPoints(player, Faction.GIANT_TRACKERS, REWARD_INTERMEDIATE * 2);
							showHtmlFile(player, "giantchaser_officer_q0843_13a.htm", npc);
							break;
						}
						
						showHtmlFile(player, "giantchaser_officer_q0843_14a.htm", npc);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs != null)
		{
			switch (qs.getCond())
			{
				case 2:
				{
					int kills = qs.getInt(Integer.toString(QUEST_ID));
					if (kills < COUNT_BASIC)
					{
						kills++;
						qs.set(Integer.toString(QUEST_ID), kills);
						
						final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
						log.addNpcString(NpcStringId.ELIMINATE_THE_MARRED_GIANT, qs.getInt(Integer.toString(QUEST_ID))); // Eliminate the Marred Giant
						qs.getPlayer().sendPacket(log);
						
						if (qs.getInt(Integer.toString(QUEST_ID)) == COUNT_BASIC)
						{
							qs.setCond(4, true);
							break;
						}
						
						playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case 3:
				{
					int kills = qs.getInt(Integer.toString(QUEST_ID));
					if (kills < COUNT_INTERMEDIATE)
					{
						kills++;
						qs.set(Integer.toString(QUEST_ID), kills);
						
						final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
						log.addNpcString(NpcStringId.ELIMINATE_THE_MARRED_GIANT, qs.getInt(Integer.toString(QUEST_ID))); // Eliminate the Marred Giant
						qs.getPlayer().sendPacket(log);
						
						if (qs.getInt(Integer.toString(QUEST_ID)) == COUNT_INTERMEDIATE)
						{
							qs.setCond(5, true);
							break;
						}
						
						playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
		}
	}
}
