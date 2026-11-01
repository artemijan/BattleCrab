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
package quests.Q10819_ForHonor;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.ceremonyofchaos.OnCeremonyOfChaosMatchResult;
import org.l2jmobius.gameserver.model.events.holders.olympiad.OnOlympiadMatchResult;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10817_ExaltedOneWhoOvercomesTheLimit.Q10817_ExaltedOneWhoOvercomesTheLimit;

/**
 * For Honor (10819)
 * @URL https://l2wiki.com/For_Honor
 * @author Mobius
 */
public class Q10819_ForHonor extends Quest
{
	// NPC
	private static final int OLYMPIAD_MANAGER = 31688;
	
	// Items
	private static final int PROOF_OF_BATTLE = 45872;
	private static final int ISHUMA_CERTIFICATE = 45630;
	private static final int SIR_KRISTOF_RODEMAI_CERTIFICATE = 45631;
	private static final int DAICHIR_SERTIFICATE = 45628;
	
	// Rewards
	private static final int OLYMPIAD_MANAGER_CERTIFICATE = 45629;
	private static final int BATTLE_QUICK_HEALING_POTION = 45945;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	public Q10819_ForHonor()
	{
		super(10819);
		addStartNpc(OLYMPIAD_MANAGER);
		addTalkId(OLYMPIAD_MANAGER);
		addCondMinLevel(MIN_LEVEL, "31688-02.html");
		addCondStartedQuest(Q10817_ExaltedOneWhoOvercomesTheLimit.class.getSimpleName(), "31688-03.html");
		registerQuestItems(PROOF_OF_BATTLE);
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
			case "31688-04.htm":
			case "31688-05.htm":
			{
				htmltext = event;
				break;
			}
			case "31688-06.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "31688-09.html":
			{
				if (qs.isCond(2))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						if (hasQuestItems(player, DAICHIR_SERTIFICATE, ISHUMA_CERTIFICATE, SIR_KRISTOF_RODEMAI_CERTIFICATE))
						{
							htmltext = "31688-10.html";
						}
						else
						{
							htmltext = event;
						}
						
						takeItems(player, PROOF_OF_BATTLE, -1);
						giveItems(player, BATTLE_QUICK_HEALING_POTION, 180);
						giveItems(player, OLYMPIAD_MANAGER_CERTIFICATE, 1);
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
				htmltext = "31688-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "31688-07.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "31688-08.html";
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
	
	private void manageQuestProgress(Player player)
	{
		if (player != null)
		{
			final QuestState qs = getQuestState(player, false);
			if ((qs != null) && qs.isCond(1))
			{
				giveItems(player, PROOF_OF_BATTLE, 1);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				if (getQuestItemsCount(player, PROOF_OF_BATTLE) >= 100)
				{
					qs.setCond(2, true);
				}
			}
		}
	}
	
	@RegisterEvent(EventType.ON_CEREMONY_OF_CHAOS_MATCH_RESULT)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	private void onCeremonyOfChaosMatchResult(OnCeremonyOfChaosMatchResult event)
	{
		event.getMembers().forEach(player -> manageQuestProgress(player));
	}
	
	@RegisterEvent(EventType.ON_OLYMPIAD_MATCH_RESULT)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	private void onOlympiadMatchResult(OnOlympiadMatchResult event)
	{
		manageQuestProgress(event.getWinner().getPlayer());
		manageQuestProgress(event.getLoser().getPlayer());
	}
}
