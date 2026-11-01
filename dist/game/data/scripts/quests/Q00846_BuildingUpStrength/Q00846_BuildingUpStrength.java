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
package quests.Q00846_BuildingUpStrength;

import java.util.HashSet;
import java.util.Set;

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

import quests.Q10845_BloodyBattleRescueTheSmiths.Q10845_BloodyBattleRescueTheSmiths;
import quests.not_done.Q00840_RequestFromTheKingdomsRoyalGuard;

/**
 * Building up Strength (846)
 * @author Kazumi
 */
public final class Q00846_BuildingUpStrength extends Quest
{
	// NPCs
	private static final int DINFORD = 34236;
	private static final int CAPTIVE_MINER = 34247;
	private static final int CAPTIVE_SCOUT = 34249;
	private static final int CAPTIVE_CHEF = 34251;
	private static final int CAPTIVE_WIZARD = 34253;
	private static final int CAPTIVE_CITIZEN = 34255;
	
	// Items
	private static final int SUPPLY_BOX_BASIC = 47184;
	private static final int SUPPLY_BOX_INTERMEDIATE = 47185;
	private static final int SUPPLY_BOX_ADVANCED = 47186;
	private static final int FACTION_AMITY_TOKEN = 48030;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	private static final int COUNT_BASIC = 2;
	private static final int COUNT_INTER = 4;
	private static final int COUNT_HIGH = 6;
	
	public Q00846_BuildingUpStrength()
	{
		super(846);
		addStartNpc(DINFORD);
		addTalkId(DINFORD);
		addCondMinLevel(MIN_LEVEL, "roayl_quartermaster_q0846_02.htm");
		addFactionLevel(Faction.KINGDOM_ROYAL_GUARDS, 3, "roayl_quartermaster_q0846_02a.htm");
		addCondCompletedQuest(Q10845_BloodyBattleRescueTheSmiths.class.getSimpleName(), "roayl_quartermaster_q0846_02.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		switch (event)
		{
			case "roayl_quartermaster_q0846_07.htm":
			case "roayl_quartermaster_q0846_07a.htm":
			{
				htmltext = event;
				break;
			}
			case "quest_accept":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 3) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 7))
				{
					htmltext = "roayl_quartermaster_q0846_05.htm";
					break;
				}
				else if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 7) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 9))
				{
					htmltext = "roayl_quartermaster_q0846_05a.htm";
					break;
				}
				
				htmltext = "roayl_quartermaster_q0846_05b.htm";
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
				if (player.getLevel() >= MIN_LEVEL)
				{
					if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 3)
					{
						htmltext = "roayl_quartermaster_q0846_01.htm";
						break;
					}
					
					htmltext = "royal_maestre_q0845_02a.htm";
					break;
				}
				
				htmltext = "roayl_quartermaster_q0846_02.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 3) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 7))
						{
							htmltext = "roayl_quartermaster_q0846_06.htm";
							break;
						}
						else if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 7) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 9))
						{
							htmltext = "roayl_quartermaster_q0846_06a.htm";
							break;
						}
						
						htmltext = "roayl_quartermaster_q0846_06b.htm";
						break;
					}
					case 2:
					{
						htmltext = "roayl_quartermaster_q0846_11.htm";
						break;
					}
					case 3:
					{
						htmltext = "roayl_quartermaster_q0846_11a.htm";
						break;
					}
					case 4:
					{
						htmltext = "roayl_quartermaster_q0846_11b.htm";
						break;
					}
					case 5:
					{
						htmltext = "roayl_quartermaster_q0846_12.htm";
						break;
					}
					case 6:
					{
						htmltext = "roayl_quartermaster_q0846_12a.htm";
						break;
					}
					case 7:
					{
						htmltext = "roayl_quartermaster_q0846_12b.htm";
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
	@Id(DINFORD)
	@Id(CAPTIVE_MINER)
	@Id(CAPTIVE_SCOUT)
	@Id(CAPTIVE_CHEF)
	@Id(CAPTIVE_WIZARD)
	@Id(CAPTIVE_CITIZEN)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 846)
		{
			switch (reply)
			{
				case 1:
				{
					switch (npc.getId())
					{
						case DINFORD:
						{
							showHtmlFile(player, "roayl_quartermaster_q0846_03.htm", npc);
							break;
						}
						case CAPTIVE_MINER:
						{
							switch (qs.getCond())
							{
								case 2:
								{
									int captiveMiner = qs.getInt(Integer.toString(CAPTIVE_MINER));
									if (qs.getInt(Integer.toString(CAPTIVE_MINER)) < COUNT_BASIC)
									{
										captiveMiner++;
										qs.set(Integer.toString(CAPTIVE_MINER), captiveMiner);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 3:
								{
									int captiveMiner = qs.getInt(Integer.toString(CAPTIVE_MINER));
									if (qs.getInt(Integer.toString(CAPTIVE_MINER)) < COUNT_INTER)
									{
										captiveMiner++;
										qs.set(Integer.toString(CAPTIVE_MINER), captiveMiner);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 4:
								{
									int captiveMiner = qs.getInt(Integer.toString(CAPTIVE_MINER));
									if (qs.getInt(Integer.toString(CAPTIVE_MINER)) < COUNT_HIGH)
									{
										captiveMiner++;
										qs.set(Integer.toString(CAPTIVE_MINER), captiveMiner);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
							}
							
							npc.doDie(player);
							addSpawn(34248, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Miner
							break;
						}
						case CAPTIVE_SCOUT:
						{
							switch (qs.getCond())
							{
								case 2:
								{
									int captiveScout = qs.getInt(Integer.toString(CAPTIVE_SCOUT));
									if (qs.getInt(Integer.toString(CAPTIVE_SCOUT)) < COUNT_BASIC)
									{
										captiveScout++;
										qs.set(Integer.toString(CAPTIVE_SCOUT), captiveScout);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 3:
								{
									int captiveScout = qs.getInt(Integer.toString(CAPTIVE_SCOUT));
									if (qs.getInt(Integer.toString(CAPTIVE_SCOUT)) < COUNT_INTER)
									{
										captiveScout++;
										qs.set(Integer.toString(CAPTIVE_SCOUT), captiveScout);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 4:
								{
									int captiveScout = qs.getInt(Integer.toString(CAPTIVE_SCOUT));
									if (qs.getInt(Integer.toString(CAPTIVE_SCOUT)) < COUNT_HIGH)
									{
										captiveScout++;
										qs.set(Integer.toString(CAPTIVE_SCOUT), captiveScout);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
							}
							
							npc.doDie(player);
							addSpawn(34250, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Scout
							break;
						}
						case CAPTIVE_CHEF:
						{
							switch (qs.getCond())
							{
								case 2:
								{
									int captiveChef = qs.getInt(Integer.toString(CAPTIVE_CHEF));
									if (qs.getInt(Integer.toString(CAPTIVE_CHEF)) < COUNT_BASIC)
									{
										captiveChef++;
										qs.set(Integer.toString(CAPTIVE_CHEF), captiveChef);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 3:
								{
									int captiveChef = qs.getInt(Integer.toString(CAPTIVE_CHEF));
									if (qs.getInt(Integer.toString(CAPTIVE_CHEF)) < COUNT_INTER)
									{
										captiveChef++;
										qs.set(Integer.toString(CAPTIVE_CHEF), captiveChef);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 4:
								{
									int captiveChef = qs.getInt(Integer.toString(CAPTIVE_CHEF));
									if (qs.getInt(Integer.toString(CAPTIVE_CHEF)) < COUNT_HIGH)
									{
										captiveChef++;
										qs.set(Integer.toString(CAPTIVE_CHEF), captiveChef);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
							}
							
							npc.doDie(player);
							addSpawn(34252, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Chef
							break;
						}
						case CAPTIVE_WIZARD:
						{
							switch (qs.getCond())
							{
								case 2:
								{
									int captiveWizard = qs.getInt(Integer.toString(CAPTIVE_WIZARD));
									if (qs.getInt(Integer.toString(CAPTIVE_WIZARD)) < COUNT_BASIC)
									{
										captiveWizard++;
										qs.set(Integer.toString(CAPTIVE_WIZARD), captiveWizard);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 3:
								{
									int captiveWizard = qs.getInt(Integer.toString(CAPTIVE_WIZARD));
									if (qs.getInt(Integer.toString(CAPTIVE_WIZARD)) < COUNT_INTER)
									{
										captiveWizard++;
										qs.set(Integer.toString(CAPTIVE_WIZARD), captiveWizard);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 4:
								{
									int captiveWizard = qs.getInt(Integer.toString(CAPTIVE_WIZARD));
									if (qs.getInt(Integer.toString(CAPTIVE_WIZARD)) < COUNT_HIGH)
									{
										captiveWizard++;
										qs.set(Integer.toString(CAPTIVE_WIZARD), captiveWizard);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
							}
							
							npc.doDie(player);
							addSpawn(34254, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Wizard
							break;
						}
						case CAPTIVE_CITIZEN:
						{
							switch (qs.getCond())
							{
								case 2:
								{
									int captiveCitizen = qs.getInt(Integer.toString(CAPTIVE_CITIZEN));
									if (qs.getInt(Integer.toString(CAPTIVE_CITIZEN)) < COUNT_BASIC)
									{
										captiveCitizen++;
										qs.set(Integer.toString(CAPTIVE_CITIZEN), captiveCitizen);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 3:
								{
									int captiveCitizen = qs.getInt(Integer.toString(CAPTIVE_CITIZEN));
									if (qs.getInt(Integer.toString(CAPTIVE_CITIZEN)) < COUNT_INTER)
									{
										captiveCitizen++;
										qs.set(Integer.toString(CAPTIVE_CITIZEN), captiveCitizen);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
								case 4:
								{
									int captiveCitizen = qs.getInt(Integer.toString(CAPTIVE_CITIZEN));
									if (qs.getInt(Integer.toString(CAPTIVE_CITIZEN)) < COUNT_HIGH)
									{
										captiveCitizen++;
										qs.set(Integer.toString(CAPTIVE_CITIZEN), captiveCitizen);
										playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
										sendNpcLogList(player);
									}
									break;
								}
							}
							
							npc.doDie(player);
							addSpawn(34256, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Citizen
							break;
						}
					}
					break;
				}
				case 2:
				{
					showHtmlFile(player, "roayl_quartermaster_q0846_04.htm", npc);
					break;
				}
				case 10:
				{
					if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 3) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 7))
					{
						showHtmlFile(player, "roayl_quartermaster_q0846_08.htm", npc);
						break;
					}
					else if ((player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 7) && (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) < 9))
					{
						showHtmlFile(player, "roayl_quartermaster_q0846_08a.htm", npc);
						break;
					}
					
					showHtmlFile(player, "roayl_quartermaster_q0846_08b.htm", npc);
					break;
				}
				case 11:
				{
					showHtmlFile(player, "roayl_quartermaster_q0846_09.htm", npc);
					break;
				}
				case 12:
				{
					showHtmlFile(player, "roayl_quartermaster_q0846_09a.htm", npc);
					break;
				}
				case 13:
				{
					showHtmlFile(player, "roayl_quartermaster_q0846_09b.htm", npc);
					break;
				}
				case 21:
				{
					qs.setCond(2);
					showHtmlFile(player, "roayl_quartermaster_q0846_10.htm", npc);
					break;
				}
				case 22:
				{
					qs.setCond(3);
					showHtmlFile(player, "roayl_quartermaster_q0846_10a.htm", npc);
					break;
				}
				case 23:
				{
					qs.setCond(4);
					showHtmlFile(player, "roayl_quartermaster_q0846_10b.htm", npc);
					break;
				}
				case 3101:
				{
					if (qs.getCond() == 5)
					{
						final Quest qs840 = ScriptManager.getInstance().getScript(Q00840_RequestFromTheKingdomsRoyalGuard.class.getSimpleName());
						if (qs840 != null)
						{
							qs840.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
						}
						
						qs.exitQuest(QuestType.DAILY, true);
						giveItems(player, SUPPLY_BOX_BASIC, 1);
						addExpAndSp(player, 7262301690L, 1429400);
						addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 100);
					}
					break;
				}
				case 3102:
				{
					if (qs.getCond() == 5)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							final Quest qs840 = ScriptManager.getInstance().getScript(Q00840_RequestFromTheKingdomsRoyalGuard.class.getSimpleName());
							if (qs840 != null)
							{
								qs840.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
							}
							
							qs.exitQuest(QuestType.DAILY, true);
							takeItems(player, FACTION_AMITY_TOKEN, 1);
							giveItems(player, SUPPLY_BOX_BASIC, 1);
							addExpAndSp(player, 7262301690L * 2, 17429400 * 2);
							addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 100 * 2);
							break;
						}
						
						showHtmlFile(player, "roayl_quartermaster_q0846_14.htm", npc);
					}
					break;
				}
				case 3201:
				{
					if (qs.getCond() == 6)
					{
						final Quest qs840 = ScriptManager.getInstance().getScript(Q00840_RequestFromTheKingdomsRoyalGuard.class.getSimpleName());
						if (qs840 != null)
						{
							qs840.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
						}
						
						qs.exitQuest(QuestType.DAILY, true);
						giveItems(player, SUPPLY_BOX_INTERMEDIATE, 1);
						addExpAndSp(player, 14524603380L, 34858800);
						addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 200);
					}
					break;
				}
				case 3202:
				{
					if (qs.getCond() == 6)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							final Quest qs840 = ScriptManager.getInstance().getScript(Q00840_RequestFromTheKingdomsRoyalGuard.class.getSimpleName());
							if (qs840 != null)
							{
								qs840.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
							}
							
							qs.exitQuest(QuestType.DAILY, true);
							takeItems(player, FACTION_AMITY_TOKEN, 1);
							giveItems(player, SUPPLY_BOX_INTERMEDIATE, 1);
							addExpAndSp(player, 14524603380L * 2, 34858800 * 2);
							addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 200 * 2);
							break;
						}
						
						showHtmlFile(player, "roayl_quartermaster_q0846_14a.htm", npc);
					}
					break;
				}
				case 3301:
				{
					if (qs.getCond() == 7)
					{
						final Quest qs840 = ScriptManager.getInstance().getScript(Q00840_RequestFromTheKingdomsRoyalGuard.class.getSimpleName());
						if (qs840 != null)
						{
							qs840.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
						}
						
						qs.exitQuest(QuestType.DAILY, true);
						giveItems(player, SUPPLY_BOX_ADVANCED, 1);
						addExpAndSp(player, 21786905070L, 52288200);
						addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 300);
					}
					break;
				}
				case 3302:
				{
					if (qs.getCond() == 7)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							final Quest qs840 = ScriptManager.getInstance().getScript(Q00840_RequestFromTheKingdomsRoyalGuard.class.getSimpleName());
							if (qs840 != null)
							{
								qs840.notifyEvent("NOTIFY_QUEST_DONE", npc, player);
							}
							
							qs.exitQuest(QuestType.DAILY, true);
							takeItems(player, FACTION_AMITY_TOKEN, 1);
							giveItems(player, SUPPLY_BOX_ADVANCED, 1);
							addExpAndSp(player, 21786905070L * 2, 52288200 * 2);
							addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, 300 * 2);
							break;
						}
						
						showHtmlFile(player, "roayl_quartermaster_q0846_14b.htm", npc);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.isCond(2) || qs.isCond(3) || qs.isCond(4)))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(84605, true, qs.getInt("captiveMiner"))); // Rescue the Captive Miners
			holder.add(new NpcLogListHolder(84606, true, qs.getInt("captiveScout"))); // Rescue the Captive Scouts
			holder.add(new NpcLogListHolder(84607, true, qs.getInt("captiveChef"))); // Rescue the Captive Chefs
			holder.add(new NpcLogListHolder(84608, true, qs.getInt("captiveWizard"))); // Rescue the Captive Wizards
			holder.add(new NpcLogListHolder(84609, true, qs.getInt("captiveCitizen"))); // Rescue the Captive Citizens
			checkLogState(player);
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
	
	public void checkLogState(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		
		switch (qs.getCond())
		{
			case 2:
			{
				if ((qs.getInt("captiveMiner") == COUNT_BASIC) && (qs.getInt("captiveScout") == COUNT_BASIC) && (qs.getInt("captiveChef") == COUNT_BASIC) && (qs.getInt("captiveWizard") == COUNT_BASIC) && (qs.getInt("captiveCitizen") == COUNT_BASIC))
				{
					qs.setCond(5, true);
				}
				break;
			}
			case 3:
			{
				if ((qs.getInt("captiveMiner") == COUNT_INTER) && (qs.getInt("captiveScout") == COUNT_INTER) && (qs.getInt("captiveChef") == COUNT_INTER) && (qs.getInt("captiveWizard") == COUNT_INTER) && (qs.getInt("captiveCitizen") == COUNT_INTER))
				{
					qs.setCond(6, true);
				}
				break;
			}
			case 4:
			{
				if ((qs.getInt("captiveMiner") == COUNT_HIGH) && (qs.getInt("captiveScout") == COUNT_HIGH) && (qs.getInt("captiveChef") == COUNT_HIGH) && (qs.getInt("captiveWizard") == COUNT_HIGH) && (qs.getInt("captiveCitizen") == COUNT_HIGH))
				{
					qs.setCond(7, true);
				}
				break;
			}
		}
	}
}
