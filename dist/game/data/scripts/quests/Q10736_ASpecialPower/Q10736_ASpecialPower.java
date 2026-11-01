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
package quests.Q10736_ASpecialPower;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10734_DoOrDie.Q10734_DoOrDie;

/**
 * A Special Power (10736)
 * @author Sdw, Trevor The Third
 */
public class Q10736_ASpecialPower extends Quest
{
	// NPC
	private static final int KATALIN = 33943;
	
	// Monsters
	private static final int FLOATO = 27526;
	private static final int FLOATO2 = 27531;
	private static final int RATEL = 27527;
	
	// Misc
	private static final int MIN_LEVEL = 4;
	private static final int MAX_LEVEL = 20;
	public static final int KILL_COUNT = 0;
	
	// Rewards
	private static final int EXP_REWARD = 3154;
	private static final int SP_REWARD = 0;
	private static final int NG_SOULSHOTS_REWARD = 1835;
	private static final int ADENA_REWARD = 900;
	
	public Q10736_ASpecialPower()
	{
		super(10736);
		addStartNpc(KATALIN);
		addTalkId(KATALIN);
		
		addCondRace(Race.ERTHEIA, "");
		addCondClassId(PlayerClass.ERTHEIA_FIGHTER, "");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33943-00.htm");
		addCondCompletedQuest(Q10734_DoOrDie.class.getSimpleName(), "33943-00.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && event.equals("33943-02.htm"))
		{
			qs.startQuest();
			return event;
		}
		
		return null;
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
				htmltext = "33943-01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "33943-03.html";
						break;
					}
					case 2:
					case 3:
					case 4:
					case 5:
					case 6:
					{
						htmltext = "33943-04.html";
						break;
					}
					case 7:
					{
						giveAdena(player, ADENA_REWARD, true);
						addExpAndSp(player, EXP_REWARD, SP_REWARD);
						giveItems(player, NG_SOULSHOTS_REWARD, 500);
						qs.exitQuest(false, true);
						htmltext = "33943-05.html";
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
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final Set<NpcLogListHolder> holder = new HashSet<>();
		final QuestState qs = getQuestState(player, false);
		if (qs != null)
		{
			int npcId = -1;
			switch (qs.getCond())
			{
				case 2:
				{
					npcId = FLOATO;
					break;
				}
				case 4:
				{
					npcId = FLOATO2;
					break;
				}
				case 6:
				{
					npcId = RATEL;
					break;
				}
			}
			
			if (npcId != -1)
			{
				holder.add(new NpcLogListHolder(npcId, false, qs.getMemoStateEx(KILL_COUNT)));
			}
		}
		
		return holder;
	}
}
