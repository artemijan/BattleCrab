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
package quests.Q10553_WhatMattersMoreThanAbility;

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
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.variables.PlayerVariables;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import instances.BalthusKnights.HatchlingCage;
import quests.Q10552_ChallengeBalthusKnight.Q10552_ChallengeBalthusKnight;

/**
 * What Matters More Than Ability (10553)
 * @author Kazumi
 */
public final class Q10553_WhatMattersMoreThanAbility extends Quest
{
	// NPCs
	private static final int STIG = 34361;
	private static final int MCCOY = 34383;
	
	// Monster
	private static final int HATCHLING = 24089;
	private static final int GEM_DRAGON = 24097;
	
	// Misc
	private static final int COUNT_HATCHLING = 5;
	private static final int COUNT_GEM_DRAGON = 4;
	
	public Q10553_WhatMattersMoreThanAbility()
	{
		super(10553);
		addStartNpc(STIG);
		addTalkId(STIG, MCCOY);
		addKillId(HATCHLING, GEM_DRAGON);
		addCondCompletedQuest(Q10552_ChallengeBalthusKnight.class.getSimpleName(), "stig_q10553_02.htm");
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
		
		if (event.equals("quest_accept"))
		{
			qs.startQuest();
			showOnScreenMsg(player, NpcStringId.TALK_TO_HATCHLING_MANAGER_MCCOY, ExShowScreenMessage.TOP_CENTER, 10000, false);
			htmltext = "stig_q10553_04.htm";
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
				if (npc.getId() == STIG)
				{
					htmltext = "stig_q10553_01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == STIG)
				{
					htmltext = "stig_q10553_05.htm";
				}
				else if (npc.getId() == MCCOY)
				{
					switch (qs.getCond())
					{
						case 1:
						{
							htmltext = "mccoy_q10553_01.htm";
							break;
						}
						case 2:
						case 3:
						{
							htmltext = "mccoy_q10553_04.htm";
							break;
						}
						case 4:
						{
							qs.exitQuest(false, true);
							showOnScreenMsg(player, NpcStringId.TALK_TO_HATCHLING_MANAGER_MCCOY_AGAIN, ExShowScreenMessage.TOP_CENTER, 10000, false);
							player.addExpAndSp(646_931_009L, 150_000L);
							htmltext = "mccoy_q10553_05.htm";
							break;
						}
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
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(STIG)
	@Id(MCCOY)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10553)
		{
			switch (reply)
			{
				case 1:
				{
					if (npc.getId() == STIG)
					{
						showHtmlFile(player, "stig_q10553_03.htm");
						break;
					}
					
					showHtmlFile(player, "mccoy_q10553_02.htm");
					break;
				}
				case 2:
				{
					qs.setCond(2);
					showHtmlFile(player, "mccoy_q10553_03.htm");
					break;
				}
				case 3:
				{
					final Quest instance = ScriptManager.getInstance().getScript(HatchlingCage.class.getSimpleName());
					if (instance != null)
					{
						instance.onEvent("enterInstance", npc, player);
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
			if (qs.isCond(2))
			{
				int killedHatchling = qs.getInt("killed_" + HATCHLING);
				
				if (npc.getId() == HATCHLING)
				{
					if (killedHatchling < COUNT_HATCHLING)
					{
						qs.set("killed_" + HATCHLING, ++killedHatchling);
						playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					
					sendNpcLogList(player);
				}
				
				if (killedHatchling >= COUNT_HATCHLING)
				{
					if (player.getVariables().getInt(PlayerVariables.BALTHUS_CAGE_EXP, 0) == 0)
					{
						player.getVariables().set(PlayerVariables.BALTHUS_CAGE_EXP, 1);
						player.addExpAndSp(578_317_523L, 100_000L);
					}
					
					qs.setCond(3, true);
				}
			}
			else if (qs.isCond(3))
			{
				int killedGemDragon = qs.getInt("killed_" + GEM_DRAGON);
				
				if (npc.getId() == GEM_DRAGON)
				{
					if (killedGemDragon < COUNT_GEM_DRAGON)
					{
						qs.set("killed_" + GEM_DRAGON, ++killedGemDragon);
						playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					
					sendNpcLogList(player);
				}
				
				if (killedGemDragon >= COUNT_GEM_DRAGON)
				{
					qs.setCond(4, true);
				}
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs != null)
		{
			if (qs.isCond(2))
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(HATCHLING, false, qs.getInt("killed_" + HATCHLING)));
				return holder;
			}
			else if (qs.isCond(3))
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(GEM_DRAGON, false, qs.getInt("killed_" + GEM_DRAGON)));
				return holder;
			}
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
