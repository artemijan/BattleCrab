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
package quests.Q00937_ToReviveTheFishingGuild;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerFishing;
import org.l2jmobius.gameserver.model.fishing.FishingEndReason;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

/**
 * To Revive The Fishing Guild (937)
 * @author Gigi
 * @date 2017-04-23 - [20:42:23]
 */
public class Q00937_ToReviveTheFishingGuild extends Quest
{
	// NPCs
	private static final int OFULLE = 31572;
	private static final int LINNAEUS = 31577;
	private static final int PERELIN = 31563;
	private static final int BLEAKER = 31567;
	private static final int CYANO = 31569;
	private static final int PAMFUS = 31568;
	private static final int LANOSCO = 31570;
	private static final int HUFS = 31571;
	private static final int MONAKAN = 31573;
	private static final int BERIX = 31576;
	private static final int LITULON = 31575;
	private static final int WILLIE = 31574;
	private static final int HILGENDORF = 31578;
	private static final int PLATIS = 31696;
	private static final int KLAUS = 31579;
	private static final int BATIDAE = 31989;
	private static final int EINDARKNER = 31697;
	private static final int GALBA = 32007;
	private static final int SANTIAGO = 34138;
	
	// Reward
	private static final int BASIC_SUPPLY_BOX = 47571;
	private static final int INTERMEDIATE_SUPPLY_BOX = 47572;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final String COUNT_VAR = "FishWinCount";
	
	public Q00937_ToReviveTheFishingGuild()
	{
		super(937);
		addStartNpc(OFULLE, LINNAEUS, PERELIN, BLEAKER, CYANO, PAMFUS, LANOSCO, HUFS, MONAKAN, BERIX, LITULON, WILLIE, HILGENDORF, PLATIS, KLAUS, BATIDAE, EINDARKNER, GALBA, SANTIAGO);
		addTalkId(OFULLE, LINNAEUS, PERELIN, BLEAKER, CYANO, PAMFUS, LANOSCO, HUFS, MONAKAN, BERIX, LITULON, WILLIE, HILGENDORF, PLATIS, KLAUS, BATIDAE, EINDARKNER, GALBA, SANTIAGO);
		addCondMinLevel(MIN_LEVEL, "noLevel.htm");
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
			case "Guild-02.htm":
			case "Guild-03.htm":
			{
				htmltext = event;
				break;
			}
			case "Guild-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "Guild-07.html":
			{
				if (qs.isCond(2))
				{
					if (player.getFactionLevel(Faction.FISHING_GUILD) <= 2) // Fisher Guild Lvl: 2
					{
						giveItems(player, BASIC_SUPPLY_BOX, 1);
					}
					else if (player.getFactionLevel(Faction.FISHING_GUILD) > 2)
					{
						giveItems(player, INTERMEDIATE_SUPPLY_BOX, 1);
					}
					
					addFactionPoints(player, Faction.FISHING_GUILD, 100);
					qs.exitQuest(QuestType.REPEATABLE, true);
					htmltext = event;
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
				htmltext = "Guild-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "Guild-05.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "Guild-06.html";
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
	
	@RegisterEvent(EventType.ON_PLAYER_FISHING)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerFishing(OnPlayerFishing event)
	{
		if (event.getReason() == FishingEndReason.WIN)
		{
			final Player player = event.getPlayer();
			final QuestState qs = getQuestState(player, false);
			if ((qs != null) && qs.isCond(1))
			{
				int count = qs.getInt(COUNT_VAR);
				qs.set(COUNT_VAR, ++count);
				if (count >= 100)
				{
					qs.setCond(2, true);
				}
				else
				{
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
				
				sendNpcLogList(player);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final int Count = qs.getInt(COUNT_VAR);
			if (Count > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(NpcStringId.FISHING, Count));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
