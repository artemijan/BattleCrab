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
package quests.Q10767_AWholeNewLevelOfAlchemy;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.item.OnItemCreate;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * A Whole New Level of Alchemy (10767)
 * @URL https://l2wiki.com/A_Whole_New_Level_of_Alchemy
 * @author Gigi
 */
public class Q10767_AWholeNewLevelOfAlchemy extends Quest
{
	// NPC
	private static final int VERUTI = 33977;
	
	// Items
	private static final int SUPERIOR_WINDY_HEALING_POTION = 39469;
	private static final int SUPERIOR_WINDY_QUIK_HEALING_POTION = 39474;
	private static final int HIGH_GRADE_LOVE_POTION = 39479;
	
	// Reward
	private static final int EXP_REWARD = 14819175;
	private static final int SP_REWARD = 3556;
	private static final int ALCHEMIC_TOME_POTION = 39482;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q10767_AWholeNewLevelOfAlchemy()
	{
		super(10767);
		addStartNpc(VERUTI);
		addTalkId(VERUTI);
		registerQuestItems(SUPERIOR_WINDY_HEALING_POTION, SUPERIOR_WINDY_QUIK_HEALING_POTION, HIGH_GRADE_LOVE_POTION);
		addCondMinLevel(MIN_LEVEL, "33977-00.htm");
		addCondRace(Race.ERTHEIA, "noErtheia.html");
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
			case "33977-02.htm":
			case "33977-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33977-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33977-07.html":
			{
				if (qs.isCond(2))
				{
					takeItems(player, SUPERIOR_WINDY_HEALING_POTION, 1000);
					takeItems(player, SUPERIOR_WINDY_QUIK_HEALING_POTION, 1000);
					takeItems(player, HIGH_GRADE_LOVE_POTION, 1000);
					giveItems(player, ALCHEMIC_TOME_POTION, 3);
					addExpAndSp(player, EXP_REWARD, SP_REWARD);
					qs.exitQuest(false, true);
				}
				
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = null;
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "33977-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "33977-05.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "33977-06.html";
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
	
	@RegisterEvent(EventType.ON_ITEM_CREATE)
	@RegisterType(ListenerRegisterType.ITEM)
	@Id(SUPERIOR_WINDY_HEALING_POTION)
	@Id(SUPERIOR_WINDY_QUIK_HEALING_POTION)
	@Id(HIGH_GRADE_LOVE_POTION)
	public void onItemCreate(OnItemCreate event)
	{
		final Player player = event.getActiveChar().asPlayer();
		if (player != null)
		{
			final QuestState qs = getQuestState(player, false);
			if ((qs != null) && (qs.isCond(1)) && (getQuestItemsCount(qs.getPlayer(), SUPERIOR_WINDY_HEALING_POTION) >= 1000) && (getQuestItemsCount(qs.getPlayer(), SUPERIOR_WINDY_QUIK_HEALING_POTION) >= 1000) && (getQuestItemsCount(qs.getPlayer(), HIGH_GRADE_LOVE_POTION) >= 1000))
			{
				qs.setCond(2, true);
			}
		}
	}
}
