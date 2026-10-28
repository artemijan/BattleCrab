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
package quests.Q10827_StepUpToLead;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnAttackableKill;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10823_ExaltedOneWhoShattersTheLimit.Q10823_ExaltedOneWhoShattersTheLimit;

/**
 * Step Up to Lead (10827)
 * @URL https://l2wiki.com/Step_Up_to_Lead
 * @author Mobius
 */
public class Q10827_StepUpToLead extends Quest
{
	// NPC
	private static final int GUSTAV = 30760;
	
	// Items
	private static final int MERLOT_SERTIFICATE = 46056;
	private static final int KURTIZ_CERTIFICATE = 46057;
	private static final int MAMMON_CERTIFICATE = 45635;
	
	// Rewards
	private static final int GUSTAV_CERTIFICATE = 45636;
	private static final int SPELLBOOK_FAVOR_OF_THE_EXALTED = 45870;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10827_StepUpToLead()
	{
		super(10827);
		addStartNpc(GUSTAV);
		addTalkId(GUSTAV);
		addCondMinLevel(MIN_LEVEL, "30760-02.html");
		addCondStartedQuest(Q10823_ExaltedOneWhoShattersTheLimit.class.getSimpleName(), "30760-03.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "30760-04.htm":
			case "30760-05.htm":
			{
				htmltext = event;
				break;
			}
			case "30760-06.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30760-09.html":
			{
				if (qs.isCond(2))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						if (hasQuestItems(player, KURTIZ_CERTIFICATE, MERLOT_SERTIFICATE, MAMMON_CERTIFICATE))
						{
							htmltext = "30760-10.html";
						}
						else
						{
							htmltext = event;
						}
						
						giveItems(player, GUSTAV_CERTIFICATE, 1);
						giveItems(player, SPELLBOOK_FAVOR_OF_THE_EXALTED, 1);
						
						// Give Exalted status here?
						// https://l2wiki.com/Noblesse
						player.setNobleLevel(2);
						player.broadcastUserInfo();
						
						qs.exitQuest(false, true);
					}
					else
					{
						htmltext = getNoQuestLevelRewardMsg(player);
					}
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
			case State.CREATED:
			{
				htmltext = "30760-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "30760-07.html";
				}
				else
				{
					htmltext = "30760-08.html";
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
	
	@RegisterEvent(EventType.ON_ATTACKABLE_KILL)
	@RegisterType(ListenerRegisterType.GLOBAL_MONSTERS)
	public void onAttackableKill(OnAttackableKill event)
	{
		final Player player = event.getAttacker();
		if (player == null)
		{
			return;
		}
		
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return;
		}
		
		if (player.getParty() == null)
		{
			return;
		}
		
		if (player.getParty().getLeader() != player)
		{
			return;
		}
		
		if (!event.getTarget().isRaid())
		{
			return;
		}
		
		if (event.getTarget().isRaidMinion())
		{
			return;
		}
		
		if (qs.isCond(1))
		{
			final int memo = qs.getMemoState() + 1;
			qs.setMemoState(memo);
			
			if (memo >= 30)
			{
				qs.setCond(2, true);
			}
		}
	}
}
