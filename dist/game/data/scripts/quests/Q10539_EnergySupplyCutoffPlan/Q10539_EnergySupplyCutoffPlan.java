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
package quests.Q10539_EnergySupplyCutoffPlan;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

import quests.Q10537_KamaelDisarray.Q10537_KamaelDisarray;

/**
 * Energy Supply Cutoff Plan (10539)
 * @URL https://l2wiki.com/Energy_Supply_Cutoff_Plan
 * @author Dmitri
 */
public class Q10539_EnergySupplyCutoffPlan extends Quest
{
	// NPCs
	private static final int KRENAHT = 34237;
	
	// Monsters
	private static final int MARKA = 23739;
	private static final int SCHLIEN = 23740;
	private static final int BERIMAH = 23741;
	
	// Reward
	private static final int RUNE_STONE = 39738;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10539_EnergySupplyCutoffPlan()
	{
		super(10539);
		addStartNpc(KRENAHT);
		addTalkId(KRENAHT);
		addKillId(MARKA, SCHLIEN, BERIMAH);
		addCondMinLevel(MIN_LEVEL, "34237-00.htm");
		addCondCompletedQuest(Q10537_KamaelDisarray.class.getSimpleName(), "34237-00.htm");
		addFactionLevel(Faction.GIANT_TRACKERS, 4, "34237-00.htm");
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
			case "34237-02.htm":
			case "34237-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34237-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34237-07.html":
			{
				giveItems(player, RUNE_STONE, 1); // Rune Stone
				addExpAndSp(player, 11073888000L, 26577180);
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
				htmltext = "34237-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34237-05.html";
				}
				else
				{
					htmltext = "34237-06.html";
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
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			int killedCount = qs.getInt(Integer.toString(npc.getId()));
			final int Marka = qs.getInt(Integer.toString(MARKA));
			final int Schlien = qs.getInt(Integer.toString(SCHLIEN));
			final int Berimah = qs.getInt(Integer.toString(BERIMAH));
			switch (qs.getCond())
			{
				case 1:
				{
					qs.set(Integer.toString(npc.getId()), ++killedCount);
					if ((Marka == 1) && (Schlien == 1) && (Berimah == 1))
					{
						qs.setCond(2, true);
					}
					else
					{
						sendNpcLogList(player);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final Set<NpcLogListHolder> npcLogList = new HashSet<>(3);
			npcLogList.add(new NpcLogListHolder(MARKA, false, qs.getInt(Integer.toString(MARKA))));
			npcLogList.add(new NpcLogListHolder(SCHLIEN, false, qs.getInt(Integer.toString(SCHLIEN))));
			npcLogList.add(new NpcLogListHolder(BERIMAH, false, qs.getInt(Integer.toString(BERIMAH))));
			return npcLogList;
		}
		
		return super.getNpcLogList(player);
	}
}
