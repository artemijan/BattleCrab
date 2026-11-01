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
package quests.Q10836_DisappearedClanMember;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerItemAdd;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Disappeared Clan Member (10836)
 * @URL https://l2wiki.com/Disappeared_Clan_Member
 * @author Gigi
 */
public class Q10836_DisappearedClanMember extends Quest
{
	// NPC
	private static final int ELIKIA = 34057;
	
	// Items
	private static final int BLACKBIRD_SEAL = 46132;
	private static final int BLACKBIRD_REPORT_GLENKINCHIE = 46134;
	private static final int BLACKBIRD_REPORT_HURAK = 46135;
	private static final int BLACKBIRD_REPORT_LAFFIAN = 46136;
	private static final int BLACKBIRD_REPORT_SHERRY = 46137;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	private static final int EAR = 17527;
	private static final int ELEXIR_OF_LIFE_R = 37097;
	private static final int ELEXIR_OF_MIND_R = 37098;
	
	public Q10836_DisappearedClanMember()
	{
		super(10836);
		addStartNpc(ELIKIA);
		addTalkId(ELIKIA);
		registerQuestItems(BLACKBIRD_SEAL, BLACKBIRD_REPORT_GLENKINCHIE, BLACKBIRD_REPORT_HURAK, BLACKBIRD_REPORT_LAFFIAN, BLACKBIRD_REPORT_SHERRY);
		addCondMinLevel(MIN_LEVEL, "34057-00.htm");
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
			case "34057-02.htm":
			case "34057-03.htm":
			case "34057-07.html":
			{
				htmltext = event;
				break;
			}
			case "34057-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34057-08.html":
			{
				giveItems(player, EAR, 5);
				giveItems(player, ELEXIR_OF_LIFE_R, 10);
				giveItems(player, ELEXIR_OF_MIND_R, 10);
				qs.exitQuest(false, true);
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
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "34057-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34057-05.html";
				}
				else
				{
					htmltext = "34057-06.html";
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
	
	@RegisterEvent(EventType.ON_PLAYER_ITEM_ADD)
	@RegisterType(ListenerRegisterType.ITEM)
	@Id(BLACKBIRD_REPORT_GLENKINCHIE)
	@Id(BLACKBIRD_REPORT_HURAK)
	@Id(BLACKBIRD_REPORT_LAFFIAN)
	@Id(BLACKBIRD_REPORT_SHERRY)
	public void onItemAdd(OnPlayerItemAdd event)
	{
		final Player player = event.getPlayer();
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.isCond(1)) && (hasQuestItems(player, BLACKBIRD_REPORT_GLENKINCHIE)) && (hasQuestItems(player, BLACKBIRD_REPORT_HURAK)) && (hasQuestItems(player, BLACKBIRD_REPORT_LAFFIAN)) && (hasQuestItems(player, BLACKBIRD_REPORT_SHERRY)))
		{
			qs.setCond(2, true);
		}
	}
}
