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
package quests.Q10797_CrossingFate;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10796_TheEyeThatDefiedTheGods.Q10796_TheEyeThatDefiedTheGods;

/**
 * Crossing Fate (10797)
 * @URL https://l2wiki.com/Crossing_Fate
 * @author Gigi
 */
public class Q10797_CrossingFate extends Quest
{
	// NPCs
	private static final int EYE_OF_ARGOS = 31683;
	private static final int DAIMON_THE_WHITE_EYED = 27499;
	private static final int[] MONSTERS =
	{
		21294, // Canyon Antelope
		21296, // Canyon Bandersnatch
		23311, // Valley Buffalo
		23312, // Valley Grendel
		21295, // Canyon Antelope Slave
		21297, // Canyon Bandersnatch Slave
		21299, // Valley Buffalo Slave
		21304 // Valley Grendel Slave
	};
	
	// Misc
	private static final int MIN_LEVEL = 70;
	private static final int MAX_LEVEL = 75;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q10797_CrossingFate()
	{
		super(10797);
		addStartNpc(EYE_OF_ARGOS);
		addTalkId(EYE_OF_ARGOS);
		addKillId(DAIMON_THE_WHITE_EYED);
		addKillId(MONSTERS);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "no_level.html");
		addCondRace(Race.ERTHEIA, "noErtheia.html");
		addCondCompletedQuest(Q10796_TheEyeThatDefiedTheGods.class.getSimpleName(), "restriction.html");
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
			case "31683-02.htm":
			case "31683-03.htm":
			{
				htmltext = event;
				break;
			}
			case "31683-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "31683-07.html":
			{
				if (qs.isCond(3))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 306167814, 653);
						giveStoryQuestReward(npc, player);
						qs.exitQuest(QuestType.ONE_TIME, true);
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
		if (qs.isCreated())
		{
			htmltext = "31683-01.htm";
		}
		else if (qs.isCond(1) || qs.isCond(2))
		{
			htmltext = "31683-05.html";
		}
		else if (qs.isCond(3))
		{
			htmltext = "31683-06.html";
		}
		else if (qs.isCompleted())
		{
			htmltext = getAlreadyCompletedMsg(player);
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && ArrayUtil.contains(MONSTERS, npc.getId()))
		{
			int count = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++count);
			if (count >= 100)
			{
				qs.setCond(2, true);
			}
			else
			{
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
		
		if ((qs != null) && qs.isCond(2) && (npc.getId() == DAIMON_THE_WHITE_EYED))
		{
			qs.setCond(3, true);
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
				holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_THE_BEASTS_OF_THE_VALLEY_4, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
