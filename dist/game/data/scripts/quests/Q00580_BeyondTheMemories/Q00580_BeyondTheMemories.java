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
package quests.Q00580_BeyondTheMemories;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

import quests.Q00561_BasicMissionHarnakUndergroundRuins.Q00561_BasicMissionHarnakUndergroundRuins;

/**
 * Regular Barrier Maintenance (529)
 * @URL https://l2wiki.com/Regular_Barrier_Maintenance
 * @author Mobius
 */
public class Q00580_BeyondTheMemories extends Quest
{
	// NPCs
	private static final int START_NPC = 33344; // Giant's Minion Hadel
	private static final int[] MONSTERS =
	{
		22931, // Krakia Bathus
		22932, // Krakia Carcass
		22933, // Krakia Lotus
		22934, // Rakzan
		22935, // Weiss Khan
		22936, // Weiss Ele
		22937, // Bamonti
		22938, // Seknus
		23349, // Noctum
		22939, // Demonic Bathus
		22940, // Demonic Carcass
		22941, // Demonic Lotus
		22942, // Demonic Rakzan
		22943, // Demonic Weiss Khan
		22944, // Demonic Weiss Ele
		22945, // Demonic Bamonti
		22946, // Demonic Seknus
		23350, // Demonic Noctum
	};
	
	// Misc
	private static final int KILLING_NPCSTRING_ID = NpcStringId.DEFEAT_MONSTERS_IN_THE_UNDERGROUND_RUINS.getId();
	private static final QuestType QUEST_TYPE = QuestType.DAILY; // REPEATABLE, ONE_TIME, DAILY
	private static final boolean PARTY_QUEST = true;
	private static final int KILLING_COND = 1;
	private static final int FINISH_COND = 2;
	private static final int MIN_LEVEL = 85;
	
	// Location
	private static final Location HARNAK_UNDERGROUND_RUINS = new Location(-114700, 147909, -7715);
	
	// Rewards
	private static final int EXP = 231860550;
	private static final int SP = 231840;
	private static final int ADENA_AMOUNT = 505080;
	
	public Q00580_BeyondTheMemories()
	{
		super(580);
		addStartNpc(START_NPC);
		addTalkId(START_NPC);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
		addCondStartedQuest(Q00561_BasicMissionHarnakUndergroundRuins.class.getSimpleName(), "33344-01.html");
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
			case "33344-02.htm":
			case "33344-03.htm":
			{
				return event;
			}
			case "33344-04.htm":
			{
				if (qs.isCreated())
				{
					qs.startQuest();
					qs.setCond(KILLING_COND);
				}
				break;
			}
			case "gotoharnak":
			{
				// Teleport to Harnak Underground Ruins Inside
				player.teleToLocation(HARNAK_UNDERGROUND_RUINS);
				break;
			}
			case "33344-06.html":
			{
				if (qs.isCond(FINISH_COND))
				{
					// Reward.
					addExpAndSp(player, EXP, SP);
					giveAdena(player, ADENA_AMOUNT, false);
					qs.exitQuest(QUEST_TYPE, true);
				}
				break;
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
					htmltext = "33344-01.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(KILLING_COND))
					{
						htmltext = "33344-04.htm";
					}
					else if (qs.isCond(FINISH_COND))
					{
						htmltext = "33344-05.html";
					}
					break;
				}
				case State.COMPLETED:
				{
					if (qs.isNowAvailable())
					{
						qs.setState(State.CREATED);
						htmltext = "33344-01.htm";
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
			final int killedGhosts = qs.getInt("AncientGhosts") + 1;
			qs.set("AncientGhosts", killedGhosts);
			playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			if (killedGhosts >= 200)
			{
				qs.setCond(FINISH_COND, true);
			}
			
			sendNpcLogList(killer);
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(KILLING_COND))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(KILLING_NPCSTRING_ID, true, qs.getInt("AncientGhosts")));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
