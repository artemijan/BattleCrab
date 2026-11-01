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
package quests.Q10852_TheMotherTreeRevivalProject;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * The Mother Tree Revival Project (10852)
 * @URL https://l2wiki.com/The_Mother_Tree_Revival_Project
 * @author Dmitri
 */
public class Q10852_TheMotherTreeRevivalProject extends Quest
{
	// NPCs
	private static final int IRENE = 34233;
	
	// Monsters
	private static final int NYMPH_SENTINEL = 23578;
	private static final int[] ROSE =
	{
		23566, // Nymph Rose
		23567, // Nymph Rose
	};
	private static final int[] LILY =
	{
		23568, // Nymph Lily
		23569, // Nymph Lily
	};
	private static final int[] TULIP =
	{
		23570, // Nymph Tulip
		23571, // Nymph Tulip
	};
	private static final int[] COSMOS =
	{
		23572, // Nymph Cosmos
		23573, // Nymph Cosmos
	};
	
	// Items
	private static final int RUNE_STONE = 39738;
	private static final int SPELLBOOK_PEGASUS = 47150;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	public Q10852_TheMotherTreeRevivalProject()
	{
		super(10852);
		addStartNpc(IRENE);
		addTalkId(IRENE);
		addKillId(COSMOS);
		addKillId(TULIP);
		addKillId(LILY);
		addKillId(ROSE);
		addKillId(NYMPH_SENTINEL);
		addCondMinLevel(MIN_LEVEL, "34233-00.htm");
		addFactionLevel(Faction.MOTHER_TREE_GUARDIANS, 6, "34233-00.htm");
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
			case "34233-02.htm":
			case "34233-03.htm":
			case "34233-04.htm":
			case "34233-08.html":
			{
				htmltext = event;
				break;
			}
			case "34233-05.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34233-09.html":
			{
				if (qs.isCond(2))
				{
					giveItems(player, RUNE_STONE, 1);
					giveItems(player, SPELLBOOK_PEGASUS, 1);
					addExpAndSp(player, 444428559000L, 444427200);
					qs.exitQuest(QuestType.ONE_TIME, true);
					htmltext = event;
				}
				else
				{
					htmltext = getNoQuestLevelRewardMsg(player);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == IRENE)
				{
					htmltext = "34233-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case IRENE:
					{
						if (qs.isCond(1))
						{
							htmltext = "34233-06.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "34233-07.html";
						}
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
			int killedTulip = qs.getInt("killed_" + TULIP[0]);
			int killedCosmos = qs.getInt("killed_" + COSMOS[0]);
			int killedLily = qs.getInt("killed_" + LILY[0]);
			int killedRose = qs.getInt("killed_" + ROSE[0]);
			int killedSentinel = qs.getInt("killed_" + NYMPH_SENTINEL);
			if (ArrayUtil.contains(TULIP, npc.getId()))
			{
				if (killedTulip < 300)
				{
					killedTulip++;
					qs.set("killed_" + TULIP[0], killedTulip);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
			}
			else if (ArrayUtil.contains(COSMOS, npc.getId()))
			{
				if (killedCosmos < 300)
				{
					killedCosmos++;
					qs.set("killed_" + COSMOS[0], killedCosmos);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
			}
			else if (ArrayUtil.contains(LILY, npc.getId()))
			{
				if (killedLily < 300)
				{
					killedLily++;
					qs.set("killed_" + LILY[0], killedLily);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
			}
			else if (ArrayUtil.contains(ROSE, npc.getId()))
			{
				if (killedRose < 300)
				{
					killedRose++;
					qs.set("killed_" + ROSE[0], killedRose);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
			}
			else if (killedSentinel < 100)
			{
				qs.set("killed_" + NYMPH_SENTINEL, ++killedSentinel);
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			
			if ((killedTulip == 300) && (killedCosmos == 300) && (killedLily == 300) && (killedRose == 300) && (killedSentinel >= 100))
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isStarted() && qs.isCond(1))
		{
			final Set<NpcLogListHolder> npcLogList = new HashSet<>(5);
			npcLogList.add(new NpcLogListHolder(TULIP[0], false, qs.getInt("killed_" + TULIP[0])));
			npcLogList.add(new NpcLogListHolder(COSMOS[0], false, qs.getInt("killed_" + COSMOS[0])));
			npcLogList.add(new NpcLogListHolder(LILY[0], false, qs.getInt("killed_" + LILY[0])));
			npcLogList.add(new NpcLogListHolder(ROSE[0], false, qs.getInt("killed_" + ROSE[0])));
			npcLogList.add(new NpcLogListHolder(NYMPH_SENTINEL, false, qs.getInt("killed_" + NYMPH_SENTINEL)));
			return npcLogList;
		}
		
		return super.getNpcLogList(player);
	}
}
