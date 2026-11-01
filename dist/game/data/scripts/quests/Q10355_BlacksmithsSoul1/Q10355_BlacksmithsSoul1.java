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
package quests.Q10355_BlacksmithsSoul1;

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

/**
 * @author Sero
 */
public class Q10355_BlacksmithsSoul1 extends Quest
{
	// NPC
	private static final int NETI = 34095;
	private static final int TAPOY = 30499;
	private static final int SHADAI = 32347;
	private static final int ISHUMA = 32615;
	private static final int MERCHANT_OF_MAMMON = 31126;
	
	// Items
	private static final int SHADOW_INGOT = 46395;
	
	// Monsters
	private static final int BURNSTEIN = 23587;
	private static final int DARK_RIDER = 26102;
	private static final int[] NYMPH_MONSTERS =
	{
		23569, // Nymph Lily
		23583, // Nymph Lily big
		23573, // Nymph Cosmos
		23567, // Nymph Rose
		23578, // Nymph Guardian
		23570, // Nymph Tulip
		19600, // Flower Bud
		23581, // Apherus
	};
	
	// Misc
	private static final int MIN_LEVEL = 99;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q10355_BlacksmithsSoul1()
	{
		super(10355);
		addStartNpc(NETI);
		addTalkId(NETI, TAPOY, SHADAI, ISHUMA, MERCHANT_OF_MAMMON);
		registerQuestItems(SHADOW_INGOT);
		addKillId(DARK_RIDER);
		addKillId(BURNSTEIN);
		addKillId(NYMPH_MONSTERS);
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		if (event.equals("34095-01.htm"))
		{
			qs.startQuest();
			htmltext = event;
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (npc.getId())
		{
			case NETI:
			{
				switch (qs.getState())
				{
					case State.CREATED:
					{
						htmltext = "34095-00.htm";
						break;
					}
					case State.STARTED:
					{
						htmltext = "30756-09.html";
						break;
					}
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = "34227-00a.htm";
							break;
						}
					}
				}
				break;
			}
			case TAPOY:
			{
				switch (qs.getState())
				{
					case State.STARTED:
					{
						if (qs.isCond(1))
						{
							htmltext = "30499-00.htm";
							qs.setCond(2);
						}
						else
						{
							htmltext = "30499-00.htm";
						}
						break;
					}
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = "34227-00a.htm";
							break;
						}
					}
				}
				break;
			}
			case SHADAI:
			{
				switch (qs.getState())
				{
					case State.STARTED:
					{
						if (qs.isCond(2))
						{
							htmltext = "32347-00.htm";
							qs.setCond(3);
						}
						else
						{
							htmltext = "32347-00.htm";
						}
						break;
					}
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = "34227-00a.htm";
							break;
						}
					}
				}
				break;
			}
			case ISHUMA:
			{
				switch (qs.getState())
				{
					case State.STARTED:
					{
						if (qs.isCond(3))
						{
							htmltext = "32615-00.htm";
							qs.setCond(4);
						}
						else
						{
							htmltext = "32615-00.htm";
						}
						break;
					}
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = "34227-00a.htm";
							break;
						}
					}
				}
				break;
			}
			case MERCHANT_OF_MAMMON:
			{
				switch (qs.getState())
				{
					case State.STARTED:
					{
						if (qs.isCond(4))
						{
							htmltext = "31126-00.htm";
							qs.setCond(5);
						}
						else if (qs.isCond(5))
						{
							htmltext = "31126-00.htm";
						}
						else if (qs.isCond(6) && (getQuestItemsCount(player, SHADOW_INGOT) >= 5))
						{
							takeItems(player, SHADOW_INGOT, 5);
							htmltext = "31126-01.htm";
							qs.setCond(7);
						}
						else if (qs.isCond(8))
						{
							htmltext = "31126-03.htm";
							addExpAndSp(player, 26918866543L, 24226979);
							qs.exitQuest(false, true);
						}
						else
						{
							htmltext = "31126-02.htm";
						}
						break;
					}
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = "34227-00a.htm";
							break;
						}
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(5))
		{
			final int killedCount = qs.getInt(Integer.toString(npc.getId()));
			if (killedCount < 5)
			{
				qs.set(Integer.toString(npc.getId()), killedCount + 1);
			}
			
			final int killedRessurected = qs.getInt(Integer.toString(DARK_RIDER));
			final int killedLunatic = qs.getInt(Integer.toString(BURNSTEIN));
			if ((killedLunatic == 5) && (killedRessurected == 5))
			{
				qs.setCond(6, true);
			}
		}
		else if ((qs != null) && qs.isCond(7))
		{
			int count = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++count);
			if (count >= 3000)
			{
				qs.setCond(8, true);
			}
			else
			{
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		executeForEachPlayer(player, npc, isSummon, true, false);
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(5))
		{
			final Set<NpcLogListHolder> npcLogList = new HashSet<>(2);
			npcLogList.add(new NpcLogListHolder(DARK_RIDER, false, qs.getInt(Integer.toString(DARK_RIDER))));
			npcLogList.add(new NpcLogListHolder(NpcStringId.DEFEAT_COMMANDER_BURNSTEIN_2, qs.getInt(Integer.toString(BURNSTEIN))));
			return npcLogList;
		}
		else if ((qs != null) && qs.isCond(7))
		{
			final int killCount = qs.getInt(KILL_COUNT_VAR);
			if (killCount > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_MONSTERS_IN_THE_ENCHANTED_VALLEY_2, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
