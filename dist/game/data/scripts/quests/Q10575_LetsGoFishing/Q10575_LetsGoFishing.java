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
package quests.Q10575_LetsGoFishing;

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
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.Q10566_BestChoice.Q10566_BestChoice;

/**
 * Let's Go Fishing (10575)
 * @URL https://l2wiki.com/Let%27s_Go_Fishing
 * @author NightBR, Werum
 */
public class Q10575_LetsGoFishing extends Quest
{
	// NPCs
	private static final int SANTIAGO = 34138;
	
	// Items
	private static final int PRACTICE_BAIT = 46737;
	private static final int PRACTICE_FISH = 46736;
	private static final int PRACTICE_FISHING_ROD = 46738;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	private static final String COUNT_VAR = "FishWinCount";
	private static final int NPCSTRING_ID = NpcStringId.CATCH_PRACTICE_FISH.getId();
	
	// Rewards
	private static final int XP = 597699960;
	private static final int SP = 597690;
	private static final int CERTIFICATE_FROM_SANTIAGO = 48173;
	private static final int FISHING_SHOT = 38154;
	private static final int REWARD_FISHING_ROD_PACK = 46739;
	private static final int BAIT = 48537;
	
	public Q10575_LetsGoFishing()
	{
		super(10575);
		addStartNpc(SANTIAGO);
		addTalkId(SANTIAGO);
		registerQuestItems(PRACTICE_BAIT, PRACTICE_FISH, PRACTICE_FISHING_ROD);
		addCondMinLevel(MIN_LEVEL, "noLevel.htm");
		addCondStartedQuest(Q10566_BestChoice.class.getSimpleName(), "34138-99.html");
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
			case "34138-03.html":
			case "34138-04.html":
			{
				htmltext = event;
				break;
			}
			case "34138-02.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34138-05.html":
			{
				// show Service/Help/Fishing page
				player.sendPacket(new ExTutorialShowId(111));
				qs.setCond(2, true);
				giveItems(player, PRACTICE_BAIT, 50);
				giveItems(player, PRACTICE_FISHING_ROD, 1);
				htmltext = event;
				break;
			}
			case "34138-07.html":
			{
				if (qs.isCond(3))
				{
					addExpAndSp(player, XP, SP);
					giveItems(player, CERTIFICATE_FROM_SANTIAGO, 1);
					giveItems(player, FISHING_SHOT, 60);
					giveItems(player, REWARD_FISHING_ROD_PACK, 1);
					giveItems(player, BAIT, 60);
					qs.unset(COUNT_VAR);
					qs.exitQuest(QuestType.ONE_TIME, true);
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
				htmltext = "34138-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.getCond() <= 2) ? "34138-05.html" : "34138-06.html";
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
			if ((qs != null) && qs.isCond(2))
			{
				int count = qs.getInt(COUNT_VAR);
				qs.set(COUNT_VAR, ++count);
				if (count >= 5)
				{
					qs.setCond(3, true);
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
		if ((qs != null) && qs.isCond(2))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(NPCSTRING_ID, true, qs.getInt(COUNT_VAR)));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
