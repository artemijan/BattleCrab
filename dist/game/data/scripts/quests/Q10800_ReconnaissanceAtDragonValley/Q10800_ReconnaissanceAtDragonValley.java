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
package quests.Q10800_ReconnaissanceAtDragonValley;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

import quests.Q10799_StrangeThingsAfootInTheValley.Q10799_StrangeThingsAfootInTheValley;

/**
 * Reconnaissance at Dragon Valley (10800)
 * @URL https://l2wiki.com/Reconnaissance_at_Dragon_Valley
 * @author Stayway
 */
public class Q10800_ReconnaissanceAtDragonValley extends Quest
{
	// NPC
	private static final int NAMO = 33973;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23430, // Prey Drake
		23431, // Beast Drake
		23432, // Dust Drake
		23433, // Vampiric Drake
		23441, // Bloody Grave Warrior
		23442, // Dark Grave Warrior
		23443, // Dark Grave Wizard
		23444, // Dark Grave Knight
	};
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 85;
	private static final String KILL_COUNT_VAR = "KillCounts";
	
	public Q10800_ReconnaissanceAtDragonValley()
	{
		super(10800);
		addStartNpc(NAMO);
		addTalkId(NAMO);
		addKillId(MONSTERS);
		addCondRace(Race.ERTHEIA, "noErtheia.html");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "no_level.html");
		addCondCompletedQuest(Q10799_StrangeThingsAfootInTheValley.class.getSimpleName(), "restriction.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		String htmltext = null;
		switch (event)
		{
			case "33973-02.htm":
			case "33973-03.htm":
			case "33973-07.html":
			{
				htmltext = event;
				break;
			}
			case "33973-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			default:
			{
				if (qs.isCond(2) && event.startsWith("giveReward_") && (player.getLevel() >= MIN_LEVEL))
				{
					final int itemId = Integer.parseInt(event.replace("giveReward_", ""));
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					giveItems(player, itemId, 30);
					addExpAndSp(player, 543080087, 30466);
					htmltext = "33973-08.html";
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
				htmltext = "33973-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.isCond(1)) ? "33973-05.html" : "33973-06.html";
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
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			int count = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++count);
			if (count >= 200)
			{
				qs.setCond(2, true);
			}
			else
			{
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final int killCounts = qs.getInt(KILL_COUNT_VAR);
			if (killCounts > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(NpcStringId.SUBJUGATION_IN_THE_SOUTHERN_DRAGON_VALLEY_2, killCounts));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
