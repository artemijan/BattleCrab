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
package quests.Q00749_TiesWithTheGuardians;

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

/**
 * Ties with the Guardians (749)
 * @author Kazumi
 */
public final class Q00749_TiesWithTheGuardians extends Quest
{
	// NPC
	private static final int RAFINI = 34356;
	
	// Item
	private static final int FACTION_AMITY_TOKEN = 48030;
	
	// Misc
	private static final int MIN_LEVEL = 88;
	private static final int MAX_LEVEL = 100;
	private static final int REWARD_BASIC = 100;
	private static final int REWARD_INTERMEDIATE = 200;
	
	public Q00749_TiesWithTheGuardians()
	{
		super(749);
		addStartNpc(RAFINI);
		addTalkId(RAFINI);
		addCondMinLevel(MIN_LEVEL, "guardian_rapini_q0749_02.htm");
		addCondMinLevel(MAX_LEVEL, "guardian_rapini_q0749_02a.htm");
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
			case "guardian_rapini_q0749_07.htm":
			{
				// htmltext = event;
				break;
			}
			case "quest_accept":
			{
				qs.startQuest();
				if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 1)
				{
					htmltext = "guardian_leader_q10851_05a.htm";
					break;
				}
				
				htmltext = "guardian_leader_q10851_05.htm";
				break;
			}
			case "NOTIFY_Q774":
			{
				switch (qs.getCond())
				{
					case 2:
					{
						qs.setCond(4, true);
						break;
					}
					case 3:
					{
						final int questsDone = qs.getMemoStateEx(RAFINI) + 1;
						if (questsDone < 2)
						{
							qs.setMemoStateEx(RAFINI, questsDone);
							sendNpcLogList(player);
							playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						}
						else
						{
							qs.setCond(5, true);
						}
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
	@Id(RAFINI)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 749)
		{
			switch (reply)
			{
				case 1:
				{
					showHtmlFile(player, "guardian_rapini_q0749_03.htm", npc);
					break;
				}
				case 2:
				{
					showHtmlFile(player, "guardian_rapini_q0749_04.htm", npc);
					break;
				}
				case 10:
				{
					if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 1)
					{
						showHtmlFile(player, "guardian_leader_q10851_05a.htm", npc);
						break;
					}
					
					showHtmlFile(player, "guardian_leader_q10851_05.htm", npc);
					break;
				}
				case 11:
				{
					showHtmlFile(player, "guardian_rapini_q0749_09.htm", npc);
					break;
				}
				case 13:
				{
					showHtmlFile(player, "guardian_rapini_q0749_09a.htm", npc);
					break;
				}
				case 21:
				{
					qs.setCond(2, true);
					showHtmlFile(player, "guardian_rapini_q0749_10.htm", npc);
					break;
				}
				case 22:
				{
					qs.setCond(3, true);
					showHtmlFile(player, "guardian_rapini_q0749_10a.htm", npc);
					break;
				}
				case 3101:
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						if (qs.isCond(4))
						{
							qs.exitQuest(QuestType.DAILY, true);
							addExpAndSp(player, 1288579410L, 1288530);
							addFactionPoints(player, Faction.GIANT_TRACKERS, REWARD_BASIC);
							showHtmlFile(player, "guardian_rapini_q0749_13.htm", npc);
							break;
						}
						break;
					}
					
					getNoQuestLevelRewardMsg(player);
					break;
				}
				case 3102:
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						if (qs.isCond(4))
						{
							if (hasQuestItems(player, FACTION_AMITY_TOKEN))
							{
								qs.exitQuest(QuestType.DAILY, true);
								takeItems(player, FACTION_AMITY_TOKEN, 1);
								addExpAndSp(player, 1288579410L * 2, 1288530 * 2);
								addFactionPoints(player, Faction.GIANT_TRACKERS, REWARD_BASIC * 2);
								showHtmlFile(player, "guardian_rapini_q0749_13.htm", npc);
								break;
							}
							
							showHtmlFile(player, "guardian_rapini_q0749_14.htm", npc);
							break;
						}
					}
					
					getNoQuestLevelRewardMsg(player);
					break;
				}
				case 3201:
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						if (qs.isCond(5))
						{
							qs.exitQuest(QuestType.DAILY, true);
							addExpAndSp(player, 2577158820L, 2577060);
							addFactionPoints(player, Faction.GIANT_TRACKERS, REWARD_INTERMEDIATE);
							showHtmlFile(player, "guardian_rapini_q0749_13a.htm", npc);
							break;
						}
						break;
					}
					
					getNoQuestLevelRewardMsg(player);
					break;
				}
				case 3202:
				{
					if ((player.getLevel() >= MIN_LEVEL) && qs.isCond(5))
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							qs.exitQuest(QuestType.DAILY, true);
							takeItems(player, FACTION_AMITY_TOKEN, 1);
							addExpAndSp(player, 2577158820L * 2, 2577060 * 2);
							addFactionPoints(player, Faction.GIANT_TRACKERS, REWARD_INTERMEDIATE * 2);
							showHtmlFile(player, "guardian_rapini_q0749_13a.htm", npc);
							break;
						}
						
						showHtmlFile(player, "guardian_rapini_q0749_14a.htm", npc);
						break;
					}
					
					getNoQuestLevelRewardMsg(player);
					break;
				}
			}
		}
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
				// fallthrou
			}
			case State.CREATED:
			{
				if (player.getLevel() >= MIN_LEVEL)
				{
					if (player.getLevel() > MAX_LEVEL)
					{
						htmltext = "guardian_rapini_q0749_02a.htm";
						break;
					}
					
					htmltext = "guardian_rapini_q0749_01.htm";
					break;
				}
				
				htmltext = "guardian_rapini_q0749_02.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 1)
						{
							htmltext = "guardian_leader_q10851_05a.htm";
							break;
						}
						
						htmltext = "guardian_leader_q10851_05.htm";
						break;
					}
					case 2:
					{
						htmltext = "guardian_rapini_q0749_11.htm";
						break;
					}
					case 3:
					{
						htmltext = "guardian_rapini_q0749_11a.htm";
						break;
					}
					case 4:
					{
						htmltext = "guardian_rapini_q0749_12.htm";
						break;
					}
					case 5:
					{
						htmltext = "guardian_rapini_q0749_12a.htm";
						break;
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
}
