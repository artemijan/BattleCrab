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
package quests.Q10559_BeforeTheGuide;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.not_done.Q10558_HiddenInChaos;

/**
 * Before the Guide (10559)
 * @author Kazumi
 */
public final class Q10559_BeforeTheGuide extends Quest
{
	// NPCs
	private static final int MONK_OF_CHAOS = 33880;
	private static final int HERPHAH = 34362;
	
	// Items
	private static final int BALTHUS_KNIGHT_BRACLET = 48277;
	
	public Q10559_BeforeTheGuide()
	{
		super(10559);
		addStartNpc(MONK_OF_CHAOS);
		addTalkId(MONK_OF_CHAOS, HERPHAH);
		addCondCompletedQuest(Q10558_HiddenInChaos.class.getSimpleName(), "monk_chaos_q10559_02.htm");
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
			htmltext = "monk_chaos_q10559_04.htm";
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
				if (npc.getId() == MONK_OF_CHAOS)
				{
					htmltext = "monk_chaos_q10559_01.htm";
					break;
				}
				
				htmltext = "herphah_q10559_01.htm";
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == MONK_OF_CHAOS)
				{
					htmltext = "monk_chaos_q10559_05.htm";
					break;
				}
				
				htmltext = "herphah_q10559_02.htm";
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
	@Id(MONK_OF_CHAOS)
	@Id(HERPHAH)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10559)
		{
			switch (reply)
			{
				case 1:
				{
					showHtmlFile(player, "monk_chaos_q10559_03.htm");
					break;
				}
				case 2:
				{
					player.teleToLocation(146516, 26833, -2200);
					break;
				}
				case 10:
				{
					qs.exitQuest(false, true);
					player.sendPacket(new ExTutorialShowId(25));
					giveItems(player, BALTHUS_KNIGHT_BRACLET, 1);
					break;
				}
			}
		}
	}
}
