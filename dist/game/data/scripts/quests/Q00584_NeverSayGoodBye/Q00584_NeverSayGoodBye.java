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
package quests.Q00584_NeverSayGoodBye;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

import quests.Q00583_MeaningOfSurvival.Q00583_MeaningOfSurvival;

/**
 * Never Say Good Bye (00584)
 * @URL https://l2wiki.com/Never_Say_Good_Bye
 * @author NightBR
 */
public class Q00584_NeverSayGoodBye extends Quest
{
	// NPCs
	private static final int START_NPC = 30137; // Vollodos
	private static final int[] MONSTERS =
	{
		23164, // Corpse Shredder
		23168, // Swamp Tracker
		23169, // Swamp Assassin
		23170, // Swamp Watcher
		23172, // Delegate of Blood
	};
	
	// Item
	private static final int MONSTER_DROP = 48380; // Rotten Blood
	
	// Misc
	private static final int REQUIRED_DROP_COUNT = 100;
	private static final int REQUIRED_KILL_COUNT = 100;
	private static final String KILL_COUNT_VAR = "KillCount";
	private static final int KILLING_NPCSTRING_ID1 = NpcStringId.LV_86_96_NEVER_SAY_GOOD_BYE_IN_PROGRESS.getId();
	private static final int KILLING_NPCSTRING_ID2 = NpcStringId.LV_86_96_NEVER_SAY_GOOD_BYE_2.getId();
	private static final QuestType QUEST_TYPE = QuestType.DAILY; // REPEATABLE, ONE_TIME, DAILY
	private static final boolean PARTY_QUEST = true;
	private static final int KILLING_COND = 1;
	private static final int FINISH_COND = 2;
	private static final int MIN_LEVEL = 86;
	private static final int MAX_LEVEL = 96;
	
	// Rewards
	private static final int XP = 284703720;
	private static final int SP = 284700;
	private static final int REWARD_ITEM1 = 57; // ADENA
	private static final int REWARD_ITEM1_AMOUNT = 512370;
	
	public Q00584_NeverSayGoodBye()
	{
		super(584);
		addStartNpc(START_NPC);
		addTalkId(START_NPC);
		addKillId(MONSTERS);
		registerQuestItems(MONSTER_DROP);
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
		addCondMaxLevel(MAX_LEVEL, getNoQuestMsg(null));
		addCondCompletedQuest(Q00583_MeaningOfSurvival.class.getSimpleName(), "30137-08.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		switch (event)
		{
			case "30137-01.htm":
			case "30137-02.htm":
			{
				return event;
			}
			case "30137-03.htm":
			{
				qs.startQuest();
				break;
			}
			case "30137-05.html":
			{
				if (qs.isCond(FINISH_COND) && (getQuestItemsCount(player, MONSTER_DROP) >= REQUIRED_DROP_COUNT))
				{
					takeItems(player, MONSTER_DROP, -1);
					
					// Reward.
					addExpAndSp(player, XP, SP);
					rewardItems(player, REWARD_ITEM1, REWARD_ITEM1_AMOUNT);
					qs.exitQuest(QUEST_TYPE, true);
				}
				break;
			}
			default:
			{
				return null;
			}
		}
		
		return event;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		if (npc.getId() == START_NPC)
		{
			switch (qs.getState())
			{
				case State.CREATED:
				{
					htmltext = "30137-00.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(KILLING_COND))
					{
						htmltext = "30137-03.htm";
					}
					else if (qs.isCond(FINISH_COND))
					{
						// Check if Faction quest has been taken
						final QuestState st = player.getQuestState("Q00563_BasicMissionBloodySwampland");
						if ((st != null) && st.isStarted())
						{
							htmltext = "30137-04.html";
						}
						else
						{
							htmltext = "30137-06.html";
						}
					}
					break;
				}
				case State.COMPLETED:
				{
					if (qs.isNowAvailable())
					{
						qs.setState(State.CREATED);
						htmltext = "30137-00.htm";
					}
					else
					{
						htmltext = getAlreadyCompletedMsg(player, QUEST_TYPE);
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = PARTY_QUEST ? getRandomPartyMemberState(killer, -1, 3, npc) : getQuestState(killer, false);
		if ((qs != null) && qs.isCond(KILLING_COND))
		{
			final Player player = qs.getPlayer();
			giveItemRandomly(player, npc, MONSTER_DROP, 1, REQUIRED_DROP_COUNT, 1, true);
			
			final int killCount = qs.getInt(KILL_COUNT_VAR) + 1;
			if (killCount < REQUIRED_KILL_COUNT)
			{
				qs.set(KILL_COUNT_VAR, killCount);
			}
			
			if ((killCount >= REQUIRED_KILL_COUNT) && (getQuestItemsCount(player, MONSTER_DROP) >= REQUIRED_DROP_COUNT))
			{
				qs.setCond(FINISH_COND, true);
			}
			
			sendNpcLogList(player);
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(KILLING_COND))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(KILLING_NPCSTRING_ID1, true, (int) getQuestItemsCount(player, MONSTER_DROP)));
			holder.add(new NpcLogListHolder(KILLING_NPCSTRING_ID2, true, qs.getInt(KILL_COUNT_VAR)));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
