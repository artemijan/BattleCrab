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
package quests.Q10569_DeclarationOfWar;

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

/**
 * Declaration of War (10569)
 * @URL https://l2wiki.com/Declaration_of_War
 * @author Dmitri
 */
public class Q10569_DeclarationOfWar extends Quest
{
	// NPCs
	private static final int KEKROPUS = 34222;
	private static final int HISTIE = 34243;
	
	// BOSS
	private static final int RAMONA = 26143;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23774, // Delta Bathus
		23775, // Delta Carcass
		23776, // Delta Kshana
		23777, // Royal Templar
		23778, // Royal Shooter
		23779, // Royal Wizard
		23780, // Royal Templar Colonel
		23781, // Royal Sharpshooter
		23782, // Royal Archmage
		23783 // Royal Gatekeeper
	};
	private static final int[] MONSTERS2 =
	{
		23816, // Om Bathus
		23817 // Om Kshana
	};
	
	// Misc
	private static final int MIN_LEVEL = 102;
	private static final int KILLING_NPCSTRING_ID = NpcStringId.SELECT_QUEST_STAGE_15.getId(); // NpcStringId.1019685
	private static final int KILLING_NPCSTRING_ID2 = NpcStringId.SELECT_QUEST_STAGE_15.getId(); // NpcStringId.1019686
	private static final boolean PARTY_QUEST = true;
	
	// Reward
	private static final int RUNE_STONE = 39738;
	
	public Q10569_DeclarationOfWar()
	{
		super(10569);
		addStartNpc(KEKROPUS);
		addTalkId(KEKROPUS, HISTIE);
		addKillId(MONSTERS);
		addKillId(RAMONA);
		addKillId(MONSTERS2);
		addCondMinLevel(MIN_LEVEL, "34222-00.htm");
		addFactionLevel(Faction.GIANT_TRACKERS, 10, "34222-00.htm");
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
			case "34222-03.htm":
			case "34222-02.htm":
			case "34222-05.htm":
			case "34243-02.htm":
			case "34243-08.html":
			{
				htmltext = event;
				break;
			}
			case "34222-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "teleport":
			{
				player.teleToLocation(79827, 152588, 2309);
				break;
			}
			case "34243-03.htm":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34243-05.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34243-09.html":
			{
				qs.setCond(6, true);
				htmltext = event;
				break;
			}
			case "34222-07.html":
			{
				if (qs.isCond(7))
				{
					addExpAndSp(player, 444428559000L, 444427200);
					giveItems(player, RUNE_STONE, 1);
					qs.exitQuest(false, true);
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
				if (npc.getId() == KEKROPUS)
				{
					htmltext = "34222-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case KEKROPUS:
					{
						if (qs.getCond() == 1)
						{
							htmltext = "34222-04.htm";
						}
						else if (qs.getCond() == 7)
						{
							htmltext = "34222-06.html";
						}
						break;
					}
					case HISTIE:
					{
						if (qs.getCond() == 1)
						{
							htmltext = "34243-01.htm";
						}
						else if (qs.getCond() == 2)
						{
							htmltext = "34243-03.htm";
						}
						else if (qs.getCond() == 3)
						{
							htmltext = "34243-04.html";
						}
						else if (qs.getCond() == 4)
						{
							htmltext = "34243-06.html";
						}
						else if (qs.getCond() == 5)
						{
							htmltext = "34243-07.html";
						}
						else if (qs.getCond() == 6)
						{
							htmltext = "34243-09.html";
						}
						else if (qs.getCond() == 7)
						{
							htmltext = "34243-10.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					htmltext = "34222-01.htm";
					break;
				}
				
				qs.setState(State.CREATED);
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
					if (ArrayUtil.contains(MONSTERS, npc.getId()))
					{
						qs.set("AncientGhosts", killedGhosts);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						if (killedGhosts >= 1000)
						{
							qs.setCond(3, true);
						}
					}
					break;
				}
				case 6:
				{
					final int killedGhosts = qs.getInt("AncientGhosts") + 1;
					if (ArrayUtil.contains(MONSTERS2, npc.getId()))
					{
						qs.set("AncientGhosts", killedGhosts);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						if (killedGhosts >= 20)
						{
							qs.setCond(7, true);
						}
					}
					break;
				}
			}
		}
		
		if ((qs != null) && qs.isCond(4))
		{
			qs.setCond(5, true);
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs != null)
		{
			if (qs.isCond(2))
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(KILLING_NPCSTRING_ID, true, qs.getInt("AncientGhosts")));
				return holder;
			}
			else if (qs.isCond(6))
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(KILLING_NPCSTRING_ID2, true, qs.getInt("AncientGhosts")));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
