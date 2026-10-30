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
package quests.Q10389_TheVoiceOfAuthority;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

import quests.Q10388_ConspiracyBehindDoor.Q10388_ConspiracyBehindDoor;

/**
 * @author Gigi
 */
public class Q10389_TheVoiceOfAuthority extends Quest
{
	// NPCs
	private static final int RADZEN = 33803;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		22139, // Old Aristocrat's Soldier
		22140, // Zombie Worker
		22141, // Forgotten Victim
		22142, // Triol's Layperson
		22144, // Resurrected Temple Knight
		22145, // Ritual Sacrifice
		22147, // Ritual Offering
		22148, // Triol's Believer
		22154, // Ritual Sacrifice
		22155, // Triol's High Priest
	};
	
	// Misc
	private static final int MIN_LEVEL = 97;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q10389_TheVoiceOfAuthority()
	{
		super(10389);
		addCondCompletedQuest(Q10388_ConspiracyBehindDoor.class.getSimpleName(), "no_quest.html");
		addStartNpc(RADZEN);
		addTalkId(RADZEN);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "accepted.html":
			{
				qs.startQuest();
				break;
			}
			case "endquest.html":
			{
				giveAdena(player, 1302720, true);
				giveItems(player, 8067, 1);
				addExpAndSp(player, 592767000, 142264);
				qs.exitQuest(false, true);
				htmltext = "endquest.html";
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
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
			case State.CREATED:
			{
				htmltext = player.getLevel() >= MIN_LEVEL ? "start.htm" : "no_level.html";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "notcollected.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "collected.html";
				}
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
			int killCount = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++killCount);
			if (killCount >= 30)
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
			final int killCount = qs.getInt(KILL_COUNT_VAR);
			if (killCount > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(NpcStringId.ELIMINATE_THE_PAGANS_IN_THE_ANTEROOM, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
