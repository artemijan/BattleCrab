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
package quests.Q10804_TheDimensionalWarpPart4;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10803_TheDimensionalWarpPart3.Q10803_TheDimensionalWarpPart3;

/**
 * The Dimensional Warp, Part 4 (10804)
 * @URL https://l2wiki.com/The_Dimensional_Warp,_Part_4
 * @author Mobius
 */
public class Q10804_TheDimensionalWarpPart4 extends Quest
{
	// NPC
	private static final int RESED = 33974;
	
	// Monsters
	private static final int UNWORLDLY_GOLEM = 23474;
	
	// Others
	private static final int MIN_LEVEL = 99;
	private static final int DARK_ETERNAL_ENHANCEMENT_STONE = 35567;
	private static final int WARP_CRYSTAL = 39597;
	
	public Q10804_TheDimensionalWarpPart4()
	{
		super(10804);
		addStartNpc(RESED);
		addTalkId(RESED);
		addKillId(UNWORLDLY_GOLEM);
		addCondMinLevel(MIN_LEVEL, "33974-00.htm");
		addCondCompletedQuest(Q10803_TheDimensionalWarpPart3.class.getSimpleName(), "");
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
			case "33974-02.htm":
			case "33974-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33974-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33974-07.html":
			{
				if (qs.isCond(2))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 60425832857L, 0);
						giveItems(player, DARK_ETERNAL_ENHANCEMENT_STONE, 1);
						giveItems(player, WARP_CRYSTAL, 300);
						qs.exitQuest(false, true);
						htmltext = event;
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
				htmltext = "33974-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.isCond(1)) ? "33974-05.html" : "33974-06.html";
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
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final Party party = killer.getParty();
		if (party != null)
		{
			party.getMembers().forEach(p -> onKill(npc, p));
		}
		else
		{
			onKill(npc, killer);
		}
	}
	
	private void onKill(Npc npc, Player killer)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if (qs != null)
		{
			final Player player = qs.getPlayer();
			int kills = qs.getInt("killed_" + UNWORLDLY_GOLEM);
			if (kills < 100)
			{
				qs.set("killed_" + UNWORLDLY_GOLEM, ++kills);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			
			if (kills >= 100)
			{
				qs.setCond(2, true);
			}
			
			sendNpcLogList(player);
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(UNWORLDLY_GOLEM, false, qs.getInt("killed_" + UNWORLDLY_GOLEM)));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
