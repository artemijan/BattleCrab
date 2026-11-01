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
package quests.Q00824_AttackTheCommandPost;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Command Post Raid (824)
 * @URL https://l2wiki.com/Command_Post_Raid
 * @author Dmitri
 */
public class Q00824_AttackTheCommandPost extends Quest
{
	// NPCs
	private static final int DEVIANNE = 34089;
	private static final int ELIKIA = 34057;
	
	// RaidBosses
	private static final int BURNSTEIN = 26136;
	
	// Rewards
	private static final int ADEN_VANGUARD_SUPPLY_BOX = 46283;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q00824_AttackTheCommandPost()
	{
		super(824);
		addStartNpc(DEVIANNE);
		addTalkId(DEVIANNE, ELIKIA);
		addKillId(BURNSTEIN);
		addCondMinLevel(MIN_LEVEL, "34089-00.htm");
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
			case "34089-02.htm":
			case "34089-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34089-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34057-07.html":
			{
				// Rewards
				giveItems(player, ADEN_VANGUARD_SUPPLY_BOX, 1);
				addExpAndSp(player, 3954960000L, 9491880);
				qs.exitQuest(QuestType.DAILY, true);
				htmltext = event;
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
				if (npc.getId() == DEVIANNE)
				{
					htmltext = "34089-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case DEVIANNE:
					{
						if (qs.isCond(1))
						{
							htmltext = "34089-05.html";
						}
						break;
					}
					case ELIKIA:
					{
						if (qs.isCond(2))
						{
							htmltext = "34057-06.html";
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
					htmltext = "34089-01.htm";
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
		executeForEachPlayer(player, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1) && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			qs.setCond(2, true);
		}
	}
}
