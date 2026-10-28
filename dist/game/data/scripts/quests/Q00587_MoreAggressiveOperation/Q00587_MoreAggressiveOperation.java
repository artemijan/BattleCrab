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
package quests.Q00587_MoreAggressiveOperation;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * More Aggressive Operation (587)
 * @URL https://l2wiki.com/More_Aggressive_Operation
 * @author Dmitri
 */
public class Q00587_MoreAggressiveOperation extends Quest
{
	// NPCs
	private static final int HESET = 33780;
	
	// Monsters
	private static final int BERSERK_CONTROL_GOLEM = 24130;
	private static final int CONTROL_GOLEM = 23261;
	private static final int[] MOBS =
	{
		23254, // Yin Spicula
		23246, // Spicula 1
		23247, // Spicula 2
		23248, // Spicula 3
		23249, // Spicula 4
		23250, // Spicula 5
		23251, // Spicula 6
		23255, // Light Golem
		23257, // Twosword Golem
		23259, // Broken-bodied Golem
		23260, // Summoned Golem Body
		23263, // Broken Golem of Repairs
		23264, // Broken Drill Golem
		23266, // Broken Pincer Golem
		23267, // Screw Golem
	};
	
	// Items
	private static final int CONTROL_MODULE = 48383;
	
	// Misc
	private static final int MIN_LEVEL = 93;
	private static final int MAX_LEVEL = 103;
	
	public Q00587_MoreAggressiveOperation()
	{
		super(587);
		addStartNpc(HESET);
		addTalkId(HESET);
		addKillId(BERSERK_CONTROL_GOLEM, CONTROL_GOLEM);
		addKillId(MOBS);
		registerQuestItems(CONTROL_MODULE);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33780-00.htm");
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
			case "33780-02.htm":
			case "33780-03.htm":
			case "33780-07.html":
			{
				htmltext = event;
				break;
			}
			case "33780-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33780-08.html":
			{
				if (qs.isCond(2))
				{
					giveAdena(player, 587070, true);
					addExpAndSp(player, 1193302530L, 1193280);
					qs.exitQuest(QuestType.DAILY, true);
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
				if (npc.getId() == HESET)
				{
					htmltext = "33780-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case HESET:
					{
						if (qs.isCond(1))
						{
							htmltext = "33780-05.html";
						}
						else
						{
							htmltext = "33780-06.html";
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
					qs.setState(State.CREATED);
					htmltext = "33780-01.htm";
				}
				else
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			int killedAntelope = qs.getInt("killed_" + MOBS[0]);
			if (npc.getId() == CONTROL_GOLEM)
			{
				if (getRandom(100) < 50)
				{
					final Npc mob = addSpawn(BERSERK_CONTROL_GOLEM, npc.getX(), npc.getY(), npc.getZ(), 0, true, 120000);
					addAttackPlayerDesire(mob, player, 5);
				}
			}
			
			if ((npc.getId() == BERSERK_CONTROL_GOLEM) && (getQuestItemsCount(player, CONTROL_MODULE) < 4) && (getRandom(100) < 40))
			{
				giveItems(player, CONTROL_MODULE, 1);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			else if (ArrayUtil.contains(MOBS, npc.getId()))
			{
				if (killedAntelope < 400)
				{
					killedAntelope++;
					qs.set("killed_" + MOBS[0], killedAntelope);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
			}
			
			if ((killedAntelope == 400) && (getQuestItemsCount(player, CONTROL_MODULE) == 4))
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
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_MONSTERS_AROUND_THE_TENT, qs.getInt("killed_" + MOBS[0])));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
