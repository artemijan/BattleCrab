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
package quests.Q10597_EscapeToTheShadowOfTheMotherTree;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.util.ArrayUtil;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * Escape to the Shadow of the Mother Tree (10597)
 * @URL https://l2wiki.com/Escape_to_the_Shadow_of_the_Mother_Tree
 * @author Dmitri
 */
public class Q10597_EscapeToTheShadowOfTheMotherTree extends Quest
{
	// NPCs
	private static final int ASTERIOS = 34411;
	private static final int NERUPA = 34412;
	
	// Monsters
	private static final int LITHRA_APRIAS = 24127;
	private static final int NERUPA_APRIAS = 24128;
	private static final int[] MONSTERS =
	{
		24117, // Crystal Reep
		24118, // Crystal Reep
		24119, // Crystal Needle
		24120, // Crystal Needle
		24121, // Treant Blossom
		24122, // Treant Blossom
		24123, // Flush Teasle
		24124, // Flush Teasle
		24125, // Creeper Rampike
		24126, // Creeper Rampike
		24139, // Reep Child
		24140, // Needle Child
		24141, // Blossom Child
		24142 // Teasle Child
	};
	
	// Items
	private static final int SUPPLY_BOX = 48399; // Mother Tree Guardians Advanced Treasure Chest: Shadow of the Mother Tree
	
	// Misc
	private static final int KILLING_NPCSTRING_ID = NpcStringId.ELIMINATE_THE_GIANT.getId(); // NpcStringId.1019709
	private static final int MIN_LEVEL = 103;
	
	public Q10597_EscapeToTheShadowOfTheMotherTree()
	{
		super(10597);
		addStartNpc(ASTERIOS);
		addTalkId(ASTERIOS, NERUPA);
		addKillId(MONSTERS);
		addKillId(LITHRA_APRIAS, NERUPA_APRIAS);
		addCondMinLevel(MIN_LEVEL, "34411-00.htm");
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
			case "34411-02.htm":
			case "34411-03.htm":
			case "34412-02.html":
			case "34412-03.html":
			case "34412-07.html":
			{
				htmltext = event;
				break;
			}
			case "34411-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34412-04.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34412-08.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34412-10.html":
			{
				if (qs.isCond(5))
				{
					addExpAndSp(player, 81574874280L, 81574830);
					giveItems(player, SUPPLY_BOX, 1);
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
				if (npc.getId() == ASTERIOS)
				{
					htmltext = "34411-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case ASTERIOS:
					{
						if (qs.isCond(1))
						{
							htmltext = "34411-04.htm";
						}
						else if (qs.getCond() > 1)
						{
							htmltext = "34411-05.html";
						}
						break;
					}
					case NERUPA:
					{
						if (qs.isCond(1))
						{
							htmltext = "34412-01.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "34412-05.html";
						}
						else if (qs.isCond(3))
						{
							htmltext = "34412-06.html";
						}
						else if (qs.isCond(4))
						{
							htmltext = "34412-08.html";
						}
						else if (qs.isCond(5))
						{
							htmltext = "34412-09.html";
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
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.getCond() > 1) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			switch (qs.getCond())
			{
				case 2:
				{
					final int killedGhosts = qs.getInt("AncientGhosts") + 1;
					if (ArrayUtil.contains(MONSTERS, npc.getId()))
					{
						qs.set("AncientGhosts", killedGhosts);
						if (killedGhosts >= 200)
						{
							qs.setCond(3, true);
						}
						else
						{
							playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							sendNpcLogList(player);
						}
					}
					break;
				}
				case 4:
				{
					int killedCount = qs.getInt(Integer.toString(npc.getId()));
					final int lithraaprias = qs.getInt(Integer.toString(LITHRA_APRIAS));
					final int nerupaaprias = qs.getInt(Integer.toString(NERUPA_APRIAS));
					qs.set(Integer.toString(npc.getId()), ++killedCount);
					if ((lithraaprias == 10) && (nerupaaprias == 10))
					{
						qs.setCond(5, true);
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
		if ((qs != null) && (qs.getCond() > 1))
		{
			final Set<NpcLogListHolder> npcLogList = new HashSet<>(3);
			npcLogList.add(new NpcLogListHolder(KILLING_NPCSTRING_ID, true, qs.getInt("AncientGhosts")));
			// npcLogList.add(new NpcLogListHolder(NpcStringId.ELIMINATE_THE_GIANT, qs.getInt("AncientGhosts"))); // NpcStringId.1019709
			npcLogList.add(new NpcLogListHolder(LITHRA_APRIAS, false, qs.getInt(Integer.toString(LITHRA_APRIAS))));
			npcLogList.add(new NpcLogListHolder(NERUPA_APRIAS, false, qs.getInt(Integer.toString(NERUPA_APRIAS))));
			// npcLogList.add(new NpcLogListHolder(NpcStringId.DEFEAT_COMMANDER_BURNSTEIN_2, qs.getInt(Integer.toString(LITHRA_APRIAS)))); // NpcStringId.1024127
			// npcLogList.add(new NpcLogListHolder(NpcStringId.DEFEAT_COMMANDER_BURNSTEIN_2, qs.getInt(Integer.toString(NERUPA_APRIAS)))); // NpcStringId.1024128
			return npcLogList;
		}
		
		return super.getNpcLogList(player);
	}
}
