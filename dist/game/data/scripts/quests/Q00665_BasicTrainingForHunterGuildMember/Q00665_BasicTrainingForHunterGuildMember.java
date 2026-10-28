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
package quests.Q00665_BasicTrainingForHunterGuildMember;

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
import org.l2jmobius.gameserver.network.NpcStringId;

/**
 * Basic Training for Hunter Guild Member (665)
 * @URL https://l2wiki.com/Basic_Training_for_Hunter_Guild_Member
 * @author Dmitri
 */
public class Q00665_BasicTrainingForHunterGuildMember extends Quest
{
	// NPCs
	private static final int ARCTURUS = 34267;
	
	// BOSS
	private static final int[] BOSS =
	{
		19253, // Zellaka
		19254, // Pelline
		19255, // Kalios
		26102, // Dark Rider
		26136, // Burnstein
		25876, // Maliss
		25877 // Isadora
	};
	
	// Misc
	private static final boolean PARTY_QUEST = true;
	private static final int MIN_LEVEL = 85;
	
	public Q00665_BasicTrainingForHunterGuildMember()
	{
		super(665);
		addStartNpc(ARCTURUS);
		addTalkId(ARCTURUS);
		addKillId(BOSS);
		addCondMinLevel(MIN_LEVEL, "34267-00.htm");
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
			case "34267-02.htm":
			case "34267-03.htm":
			case "34267-04.htm":
			case "34267-04a.htm":
			case "34267-06.html":
			case "34267-06a.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if (player.getFactionLevel(Faction.HUNTERS_GUILD) < 4)
				{
					htmltext = "34267-04a.htm";
					break;
				}
				
				htmltext = "34267-04.htm";
				break;
			}
			case "return":
			{
				if (player.getFactionLevel(Faction.HUNTERS_GUILD) < 4)
				{
					htmltext = "34267-04a.htm";
					break;
				}
				
				htmltext = "34267-04.htm";
				break;
			}
			case "34267-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34267-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34267-10.html":
			{
				switch (qs.getCond())
				{
					case 4:
					{
						if (player.getLevel() >= MIN_LEVEL)
						{
							addFactionPoints(player, Faction.HUNTERS_GUILD, 100);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
					case 5:
					{
						if (player.getLevel() >= MIN_LEVEL)
						{
							addFactionPoints(player, Faction.HUNTERS_GUILD, 150);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
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
				htmltext = "34267-01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if (player.getFactionLevel(Faction.HUNTERS_GUILD) < 4)
						{
							htmltext = "34267-04a.htm";
							break;
						}
						
						htmltext = "34267-04.htm";
						break;
					}
					case 2:
					{
						htmltext = "34267-08.html";
						break;
					}
					case 3:
					{
						htmltext = "34267-08a.html";
						break;
					}
					case 4:
					case 5:
					{
						htmltext = "34267-09.html";
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				else
				{
					qs.setState(State.CREATED);
					htmltext = "34267-01.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = PARTY_QUEST ? getRandomPartyMemberState(killer, -1, 3, npc) : getQuestState(killer, false);
		if ((qs != null) && (qs.getCond() > 1))
		{
			switch (qs.getCond())
			{
				case 2:
				{
					final int killedGhosts = qs.getInt("AncientGhosts") + 1;
					qs.set("AncientGhosts", killedGhosts);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					if (killedGhosts >= 1)
					{
						qs.setCond(4, true);
					}
					break;
				}
				case 3:
				{
					final int killedGhosts = qs.getInt("AncientGhosts") + 1;
					qs.set("AncientGhosts", killedGhosts);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					if (killedGhosts >= 2)
					{
						qs.setCond(5, true);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.getCond() > 1))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(NpcStringId.COMPLETE_BASIC_TRAINING.getId(), true, qs.getInt("AncientGhosts")));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
