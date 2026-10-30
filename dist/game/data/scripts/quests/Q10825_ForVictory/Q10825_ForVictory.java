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
package quests.Q10825_ForVictory;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.sieges.OnCastleSiegeFinish;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10823_ExaltedOneWhoShattersTheLimit.Q10823_ExaltedOneWhoShattersTheLimit;

/**
 * For Victory (10825)
 * @URL https://l2wiki.com/For_Victory
 * @author Mobius
 */
public class Q10825_ForVictory extends Quest
{
	// NPC
	private static final int KURTIZ = 34019;
	
	// Items
	private static final int MARK_OF_VALOR = 46059;
	private static final int MERLOT_SERTIFICATE = 46056;
	private static final int MAMMON_CERTIFICATE = 45635;
	private static final int GUSTAV_CERTIFICATE = 45636;
	
	// Rewards
	private static final int KURTIZ_CERTIFICATE = 46057;
	private static final int SPELLBOOK_SUMMON_BATTLE_POTION = 45927;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10825_ForVictory()
	{
		super(10825);
		addStartNpc(KURTIZ);
		addTalkId(KURTIZ);
		addCondMinLevel(MIN_LEVEL, "30870-02.html");
		addCondStartedQuest(Q10823_ExaltedOneWhoShattersTheLimit.class.getSimpleName(), "30870-03.html");
		registerQuestItems(MARK_OF_VALOR);
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
			case "30870-04.htm":
			case "30870-05.htm":
			{
				htmltext = event;
				break;
			}
			case "30870-06.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30870-09.html":
			{
				if (qs.isCond(1) && (getQuestItemsCount(player, MARK_OF_VALOR) >= 10))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						if (hasQuestItems(player, MERLOT_SERTIFICATE, MAMMON_CERTIFICATE, GUSTAV_CERTIFICATE))
						{
							htmltext = "30870-10.html";
						}
						else
						{
							htmltext = event;
						}
						
						giveItems(player, KURTIZ_CERTIFICATE, 1);
						giveItems(player, SPELLBOOK_SUMMON_BATTLE_POTION, 1);
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
				htmltext = "30870-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (getQuestItemsCount(player, MARK_OF_VALOR) >= 10)
				{
					htmltext = "30870-08.html";
				}
				else
				{
					htmltext = "30870-07.html";
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
				giveItems(player, MARK_OF_VALOR, 1);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@RegisterEvent(EventType.ON_CASTLE_SIEGE_FINISH)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	private void onCastleSiegeFinish(OnCastleSiegeFinish event)
	{
		event.getSiege().getPlayersInZone().forEach(this::manageQuestProgress);
	}
}
