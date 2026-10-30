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
package quests.Q10552_ChallengeBalthusKnight;

import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.variables.PlayerVariables;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;
import org.l2jmobius.gameserver.network.serverpackets.ServerPacket;

import instances.BalthusKnights.HatchlingNest;

/**
 * Challenge! Balthus Knight! (10552)
 * @author Kazumi
 */
public final class Q10552_ChallengeBalthusKnight extends Quest
{
	// NPCs
	private static final int TARTI = 34359;
	private static final int STIG = 34361;
	
	// Items
	private static final int HATCHLING_SCALE = 48192;
	
	// Monster
	private static final int HATCHLING = 24089;
	
	// Skills
	private static final SkillHolder SKILL_BALTHUS_KNIGHT_MEMBER = new SkillHolder(32130, 1);
	
	// Misc
	private static final int ITEM_COUNT = 3;
	
	// TODO: Adding for retail way.
	// private static boolean _firstDrop = true;
	
	public Q10552_ChallengeBalthusKnight()
	{
		super(10552);
		addStartNpc(TARTI);
		addTalkId(TARTI, STIG);
		addKillId(HATCHLING);
		registerQuestItems(HATCHLING_SCALE);
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
			final ServerPacket packet = new ExTutorialShowId(13);
			player.sendPacket(packet);
			
			// TODO: Adding for retail way.
			// _firstDrop = true;
			final Quest instance = ScriptManager.getInstance().getScript(HatchlingNest.class.getSimpleName());
			if (instance != null)
			{
				instance.onEvent("enterInstance", npc, player);
			}
			
			htmltext = "tarti_balthus_q10552_03.htm";
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
				if (npc.getId() == TARTI)
				{
					htmltext = "tarti_balthus_q10552_01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == TARTI)
				{
					switch (qs.getCond())
					{
						case 1:
						{
							// TODO: Adding for retail way.
							// _firstDrop = true;
							takeItems(player, HATCHLING_SCALE, -1);
							final Quest instance = ScriptManager.getInstance().getScript(HatchlingNest.class.getSimpleName());
							if (instance != null)
							{
								instance.onEvent("enterInstance", npc, player);
							}
							
							htmltext = "tarti_balthus_q10552_04.htm";
							break;
						}
						case 5:
						{
							htmltext = "tarti_balthus_q10552_05.htm";
							break;
						}
					}
				}
				else if (npc.getId() == STIG)
				{
					switch (qs.getCond())
					{
						case 1:
						{
							htmltext = "stig_q10552_01.htm";
							break;
						}
						case 5:
						{
							htmltext = "stig_q10552_02.htm";
							break;
						}
					}
				}
				break;
			}
			case State.COMPLETED:
				htmltext = getAlreadyCompletedMsg(player);
				break;
		}
		
		return htmltext;
	}
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(TARTI)
	@Id(STIG)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		// final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10552)
		{
			switch (reply)
			{
				case 1:
				{
					showHtmlFile(player, "tarti_balthus_q10552_02.htm");
					break;
				}
				case 10:
				{
					if (qs.getCond() == 5)
					{
						qs.exitQuest(false, true);
						showOnScreenMsg(player, NpcStringId.TALK_TO_BALTHUS_KNIGHT_CAPTAIN_STIG_MACH_AGAIN, ExShowScreenMessage.TOP_CENTER, 10000, false);
						player.addExpAndSp(414_745_398L, 50_000L);
						player.setTarget(player);
						player.doCast(SKILL_BALTHUS_KNIGHT_MEMBER.getSkill());
						showHtmlFile(player, "stig_q10552_03.htm");
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, true);
		if ((qs != null) && qs.isCond(1))
		{
			giveItemRandomly(player, npc, HATCHLING_SCALE, 1, ITEM_COUNT, 0.5, true);
			if ((getQuestItemsCount(player, HATCHLING_SCALE) >= ITEM_COUNT))
			{
				final Instance instance = player.getInstanceWorld();
				instance.finishInstance(0);
				final ServerPacket packet = new ExTutorialShowId(11);
				player.sendPacket(packet);
				player.getVariables().set(PlayerVariables.BALTHUS_PHASE, 2);
				showOnScreenMsg(player, NpcStringId.TALK_TO_BALTHUS_KNIGHT_CAPTAIN_STIG_MACH, ExShowScreenMessage.TOP_CENTER, 10000, false);
				qs.setCond(5, true);
			}
		}
	}
}
