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
package quests.Q00841_ContaminationContainment;

import java.util.HashSet;
import java.util.Set;

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
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

import quests.Q10851_ElvenBotany.Q10851_ElvenBotany;
import quests.not_done.Q00838_RequestFromTheMotherTreeGuardians;

/**
 * Contamination Containment (841)
 * @author Kazumi
 */
public final class Q00841_ContaminationContainment extends Quest
{
	// NPCs
	private static final int IRENE = 34233;
	
	// Monster
	private static final int[] MONSTERS =
	{
		23786, // Nymph Rose - Contaminated
		23787, // Nymph Lily - Contaminated
		23788, // Nymph Tulip - Contaminated
		23789, // Nymph Cosmos - Contaminated
	};
	
	// Items
	private static final int PURIFIED_WATER = 47170;
	private static final int SUPPLY_BOX_BASIC = 47178;
	private static final int SUPPLY_BOX_INTERMEDIATE = 47179;
	private static final int SUPPLY_BOX_ADVANCED = 47180;
	private static final int FACTION_AMITY_TOKEN = 48030;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	private static final int COUNT_BASIC = 100;
	private static final int COUNT_INTERMEDIATE = 200;
	private static final int COUNT_ADVANCED = 300;
	private static final int REWARD_BASIC = 100;
	private static final int REWARD_INTERMEDIATE = 200;
	private static final int REWARD_ADVANCED = 300;
	
	public Q00841_ContaminationContainment()
	{
		super(841);
		addStartNpc(IRENE);
		addTalkId(IRENE);
		addKillId(MONSTERS);
		registerQuestItems(PURIFIED_WATER);
		addFactionLevel(Faction.MOTHER_TREE_GUARDIANS, 2, "guardian_leader_q0841_02a.htm");
		addCondMinLevel(MIN_LEVEL, "guardian_leader_q0841_02.htm");
		addCondCompletedQuest(Q10851_ElvenBotany.class.getSimpleName(), "guardian_leader_q0841_02.htm");
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
			case "guardian_leader_q0841_07.htm":
			case "guardian_leader_q0841_07a.htm":
			{
				htmltext = event;
				break;
			}
			case "quest_accept":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 2) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 4))
				{
					htmltext = "guardian_leader_q0841_05.htm";
				}
				else if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 4) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 5))
				{
					htmltext = "guardian_leader_q0841_05a.htm";
				}
				else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 5)
				{
					htmltext = "guardian_leader_q0841_05b.htm";
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
				htmltext = "guardian_leader_q0841_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 2) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 4))
						{
							htmltext = "guardian_leader_q0841_08.htm";
						}
						else if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 4) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 5))
						{
							htmltext = "guardian_leader_q0841_08a.htm";
						}
						else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 5)
						{
							htmltext = "guardian_leader_q0841_08b.htm";
						}
						break;
					}
					case 2:
					{
						htmltext = "guardian_leader_q0841_11.htm";
						break;
					}
					case 3:
					{
						htmltext = "guardian_leader_q0841_11a.htm";
						break;
					}
					case 4:
					{
						htmltext = "guardian_leader_q0841_11b.htm";
						break;
					}
					case 5:
					{
						htmltext = "guardian_leader_q0841_12.htm";
						break;
					}
					case 6:
					{
						htmltext = "guardian_leader_q0841_12a.htm";
						break;
					}
					case 7:
					{
						htmltext = "guardian_leader_q0841_12b.htm";
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
	@Id(IRENE)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 841)
		{
			switch (reply)
			{
				case 1:
				{
					showHtmlFile(player, "guardian_leader_q0841_03.htm", npc);
					break;
				}
				case 2:
				{
					showHtmlFile(player, "guardian_leader_q0841_04.htm", npc);
					break;
				}
				case 10:
				{
					if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 2) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 4))
					{
						showHtmlFile(player, "guardian_leader_q0841_06.htm", npc);
					}
					else if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 4) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 5))
					{
						showHtmlFile(player, "guardian_leader_q0841_06a.htm", npc);
					}
					else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 5)
					{
						showHtmlFile(player, "guardian_leader_q0841_06b.htm", npc);
					}
					break;
				}
				case 11:
				{
					showHtmlFile(player, "guardian_leader_q0841_09.htm", npc);
					break;
				}
				case 12:
				{
					showHtmlFile(player, "guardian_leader_q0841_09a.htm", npc);
					break;
				}
				case 13:
				{
					showHtmlFile(player, "guardian_leader_q0841_09b.htm", npc);
					break;
				}
				case 21:
				{
					qs.setCond(2);
					giveItems(player, PURIFIED_WATER, 1);
					showHtmlFile(player, "guardian_leader_q0841_10.htm", npc);
					break;
				}
				case 22:
				{
					qs.setCond(3);
					giveItems(player, PURIFIED_WATER, 1);
					showHtmlFile(player, "guardian_leader_q0841_10a.htm", npc);
					break;
				}
				case 23:
				{
					qs.setCond(4);
					giveItems(player, PURIFIED_WATER, 1);
					showHtmlFile(player, "guardian_leader_q0841_10b.htm", npc);
					break;
				}
				case 3101:
				{
					if (qs.getCond() == 5)
					{
						qs.exitQuest(QuestType.DAILY, true);
						
						final Quest qs838 = ScriptManager.getInstance().getScript(Q00838_RequestFromTheMotherTreeGuardians.class.getSimpleName());
						if (qs838 != null)
						{
							qs838.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
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
						addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, REWARD_BASIC);
						showHtmlFile(player, "guardian_leader_q0841_13.htm", npc);
					}
					break;
				}
				case 3102:
				{
					if (qs.getCond() == 5)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							qs.exitQuest(QuestType.DAILY, true);
							
							final Quest qs838 = ScriptManager.getInstance().getScript(Q00838_RequestFromTheMotherTreeGuardians.class.getSimpleName());
							if (qs838 != null)
							{
								qs838.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
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
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, REWARD_BASIC * 2);
							showHtmlFile(player, "guardian_leader_q0841_13.htm", npc);
							break;
						}
						
						showHtmlFile(player, "guardian_leader_q0841_14.htm", npc);
					}
					break;
				}
				case 3201:
				{
					if (qs.getCond() == 6)
					{
						qs.exitQuest(QuestType.DAILY, true);
						
						final Quest qs838 = ScriptManager.getInstance().getScript(Q00838_RequestFromTheMotherTreeGuardians.class.getSimpleName());
						if (qs838 != null)
						{
							qs838.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
						}
						
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
						
						addExpAndSp(player, 11073888000L, 26577180);
						addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, REWARD_INTERMEDIATE);
						showHtmlFile(player, "guardian_leader_q0841_13a.htm", npc);
					}
					break;
				}
				case 3202:
				{
					if (qs.getCond() == 6)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							qs.exitQuest(QuestType.DAILY, true);
							
							final Quest qs838 = ScriptManager.getInstance().getScript(Q00838_RequestFromTheMotherTreeGuardians.class.getSimpleName());
							if (qs838 != null)
							{
								qs838.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
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
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, REWARD_INTERMEDIATE * 2);
							showHtmlFile(player, "guardian_leader_q0841_13a.htm", npc);
							break;
						}
						
						showHtmlFile(player, "guardian_leader_q0841_14a.htm", npc);
					}
					break;
				}
				case 3301:
				{
					if (qs.getCond() == 7)
					{
						qs.exitQuest(QuestType.DAILY, true);
						
						final Quest qs838 = ScriptManager.getInstance().getScript(Q00838_RequestFromTheMotherTreeGuardians.class.getSimpleName());
						if (qs838 != null)
						{
							qs838.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
						}
						
						if (Rnd.get(100) <= 60)
						{
							giveItems(player, SUPPLY_BOX_ADVANCED, 1);
						}
						else
						{
							if (Rnd.get(100) <= 70)
							{
								giveItems(player, SUPPLY_BOX_INTERMEDIATE, 1);
							}
							else
							{
								giveItems(player, SUPPLY_BOX_BASIC, 1);
							}
						}
						
						addExpAndSp(player, 16610832000L, 39865770);
						addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, REWARD_ADVANCED);
						showHtmlFile(player, "guardian_leader_q0841_13b.htm", npc);
					}
					break;
				}
				case 3302:
				{
					if (qs.getCond() == 7)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							qs.exitQuest(QuestType.DAILY, true);
							
							final Quest qs838 = ScriptManager.getInstance().getScript(Q00838_RequestFromTheMotherTreeGuardians.class.getSimpleName());
							if (qs838 != null)
							{
								qs838.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
							}
							
							takeItems(player, FACTION_AMITY_TOKEN, 1);
							if (Rnd.get(100) <= 60)
							{
								giveItems(player, SUPPLY_BOX_ADVANCED, 1);
							}
							else
							{
								if (Rnd.get(100) <= 70)
								{
									giveItems(player, SUPPLY_BOX_INTERMEDIATE, 1);
								}
								else
								{
									giveItems(player, SUPPLY_BOX_BASIC, 1);
								}
							}
							
							addExpAndSp(player, 16610832000L * 2, 39865770 * 2);
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, REWARD_ADVANCED * 2);
							showHtmlFile(player, "guardian_leader_q0841_13b.htm", npc);
							break;
						}
						
						showHtmlFile(player, "guardian_leader_q0841_14b.htm", npc);
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
		if ((qs != null) && qs.isStarted() && qs.isCond(1))
		{
			int killedNymphs = qs.getInt("killed_Nymphs");
			switch (qs.getCond())
			{
				case 2:
				{
					if (killedNymphs >= COUNT_BASIC)
					{
						qs.setCond(5, true);
						break;
					}
					
					qs.set("killed_Nymphs", ++killedNymphs);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					sendNpcLogList(player);
					break;
				}
				case 3:
				{
					if (killedNymphs >= COUNT_INTERMEDIATE)
					{
						qs.setCond(6, true);
						break;
					}
					
					qs.set("killed_Nymphs", ++killedNymphs);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					sendNpcLogList(player);
					break;
				}
				case 4:
				{
					if (killedNymphs >= COUNT_ADVANCED)
					{
						qs.setCond(7, true);
						break;
					}
					
					qs.set("killed_Nymphs", ++killedNymphs);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					sendNpcLogList(player);
					break;
				}
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_THE_ENRAGED_NYMPH, qs.getInt("killed_Nymphs"))); // Defeat the Enraged Nymph
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		sendNpcLogList(player);
	}
}
