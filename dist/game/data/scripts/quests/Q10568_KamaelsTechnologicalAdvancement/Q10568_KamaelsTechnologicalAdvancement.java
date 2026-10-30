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
package quests.Q10568_KamaelsTechnologicalAdvancement;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

/**
 * Kamael's Technological Advancement (10568)
 * @URL https://l2wiki.com/Kamael%27s_Technological_Advancement
 * @author Dmitri
 */
public class Q10568_KamaelsTechnologicalAdvancement extends Quest
{
	// NPCs
	private static final int HISTIE = 34243;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23816, // Om Bathus
		23817 // Om Kshana
	};
	
	// Items
	private static final int HUNTERS_STONE_FRAGMENT = 48166;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	private static final int KILLING_NPCSTRING_ID = NpcStringId.DEFEAT_THE_HUNTER.getId(); // NpcStringId.1019686
	private static final boolean PARTY_QUEST = true;
	
	// Reward
	private static final int HUNTERS_STONE = 48167;
	
	public Q10568_KamaelsTechnologicalAdvancement()
	{
		super(10568);
		addStartNpc(HISTIE);
		addTalkId(HISTIE);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "34243-00.htm");
		addFactionLevel(Faction.GIANT_TRACKERS, 7, "34243-00.htm");
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
			case "34243-02.htm":
			case "34243-03.htm":
			case "34243-04.htm":
			case "34243-08.html":
			{
				htmltext = event;
				break;
			}
			case "34243-05.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34243-09.html":
			{
				giveItems(player, HUNTERS_STONE, 1);
				addExpAndSp(player, 44442855900L, 44442720);
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
				if (!hasQuestItems(player, HUNTERS_STONE_FRAGMENT, 3))
				{
					htmltext = "34243-00.htm";
					break;
				}
				
				htmltext = "34243-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34243-06.html";
				}
				else
				{
					htmltext = "34243-07.html";
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
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = PARTY_QUEST ? getRandomPartyMemberState(killer, -1, 3, npc) : getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			final int killedGhosts = qs.getInt("AncientGhosts") + 1;
			qs.set("AncientGhosts", killedGhosts);
			playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			if (killedGhosts >= 10)
			{
				qs.setCond(2, true);
			}
			
			sendNpcLogList(killer);
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(KILLING_NPCSTRING_ID, true, qs.getInt("AncientGhosts")));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
