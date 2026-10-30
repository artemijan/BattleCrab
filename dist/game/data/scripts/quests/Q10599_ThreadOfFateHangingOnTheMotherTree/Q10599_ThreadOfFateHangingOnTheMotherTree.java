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
package quests.Q10599_ThreadOfFateHangingOnTheMotherTree;

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
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10598_WithAllYourMight.Q10598_WithAllYourMight;
import quests.Q10852_TheMotherTreeRevivalProject.Q10852_TheMotherTreeRevivalProject;

/**
 * Thread of Fate Hanging on the Mother Tree (10599)
 * @URL https://l2wiki.com/Thread_of_Fate_Hanging_on_the_Mother_Tree
 * @author Dmitri
 */
public class Q10599_ThreadOfFateHangingOnTheMotherTree extends Quest
{
	// NPCs
	private static final int NERUPA = 34412;
	
	// Monsters
	private static final int BOSS = 26312; // Lithra 106
	private static final int[] MONSTERS =
	{
		24118, // Crystal Reep
		24120, // Crystal Needle
		24122, // Treant Blossom
		24124, // Flush Teasle
		24126, // Creeper Rampike
		24128 // Nerupa Aprias
	};
	
	// Items
	private static final int THREAD_OF_FATE = 48367; // Thread of Fate
	
	// Reward Items
	private static final int IMMORTAL_SCROLL_CHAPTER = 26431;
	private static final int RUNE_STONE = 39738;
	private static final int HUNTERS_AMULET = 47739;
	
	// Misc
	private static final int MIN_LEVEL = 103;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q10599_ThreadOfFateHangingOnTheMotherTree()
	{
		super(10599);
		addStartNpc(NERUPA);
		addTalkId(NERUPA);
		addKillId(MONSTERS);
		addKillId(BOSS);
		registerQuestItems(THREAD_OF_FATE);
		addCondMinLevel(MIN_LEVEL, "34412-00.htm");
		addCondCompletedQuest(Q10852_TheMotherTreeRevivalProject.class.getSimpleName(), "34412-00.htm");
		addCondCompletedQuest(Q10598_WithAllYourMight.class.getSimpleName(), "34412-00.htm");
		addFactionLevel(Faction.MOTHER_TREE_GUARDIANS, 10, "34412-00.htm");
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
			case "34412-03.htm":
			case "34412-02.htm":
			case "34412-06.html":
			{
				htmltext = event;
				break;
			}
			case "34412-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34412-07.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34412-09.html":
			{
				if (qs.isCond(5))
				{
					addExpAndSp(player, 108766499040L, 108766440);
					takeItems(player, THREAD_OF_FATE, -1);
					giveItems(player, IMMORTAL_SCROLL_CHAPTER, 1);
					giveItems(player, HUNTERS_AMULET, 1);
					giveItems(player, RUNE_STONE, 1);
					qs.exitQuest(false, true);
					htmltext = event;
					break;
				}
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
				htmltext = "34412-01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					case 2:
					{
						htmltext = "34412-04.htm";
						break;
					}
					case 3:
					{
						htmltext = "34412-05.html";
						break;
					}
					case 4:
					{
						htmltext = "34412-07.html";
						break;
					}
					case 5:
					{
						htmltext = "34412-08.html";
						break;
					}
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
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			qs.setCond(2, true);
		}
		
		if ((qs != null) && qs.isCond(2) && ArrayUtil.contains(MONSTERS, npc.getId()))
		{
			int count = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++count);
			if (count >= 30)
			{
				qs.setCond(3, true);
			}
			else
			{
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
		
		if ((qs != null) && qs.isCond(4) && (npc.getId() == BOSS))
		{
			qs.setCond(5, true);
			giveItems(killer, THREAD_OF_FATE, 1);
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(2))
		{
			final int killCount = qs.getInt(KILL_COUNT_VAR);
			if (killCount > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_REPEAT, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
