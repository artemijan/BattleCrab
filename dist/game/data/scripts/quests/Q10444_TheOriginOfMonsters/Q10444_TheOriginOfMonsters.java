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
package quests.Q10444_TheOriginOfMonsters;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

import quests.Q10443_TheAnnihilatedPlains2.Q10443_TheAnnihilatedPlains2;

/**
 * The Origin of Monsters (10444)
 * @URL https://l2wiki.com/The_Origin_of_Monsters
 * @author Gigi
 */
public class Q10444_TheOriginOfMonsters extends Quest
{
	// NPCs
	private static final int PARAJAN = 33842;
	private static final int QUINCY = 33838;
	private static final int KROGEL = 25927;
	
	// Items
	private static final int BLOODY_ETERNEL_ENHANCEMENT_STONE = 35569;
	private static final int ELMORES_SUPPORT_BOX = 37020;
	private static final int CHUNK_OF_A_CROPSE = 36679;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	public Q10444_TheOriginOfMonsters()
	{
		super(10444);
		addStartNpc(PARAJAN);
		addTalkId(PARAJAN, QUINCY);
		addKillId(KROGEL);
		registerQuestItems(CHUNK_OF_A_CROPSE);
		addCondMinLevel(MIN_LEVEL, "33842-00.htm");
		addCondCompletedQuest(Q10443_TheAnnihilatedPlains2.class.getSimpleName(), "33842-00.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = null;
		switch (event)
		{
			case "33842-02.htm":
			case "33842-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33842-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33838-02.html":
			{
				if (qs.isCond(2))
				{
					qs.exitQuest(false, true);
					giveItems(player, BLOODY_ETERNEL_ENHANCEMENT_STONE, 1);
					giveItems(player, ELMORES_SUPPORT_BOX, 1);
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
				if (npc.getId() == PARAJAN)
				{
					htmltext = "33842-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case PARAJAN:
					{
						if (qs.isCond(1))
						{
							htmltext = "33842-05.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "33842-06.html";
						}
						break;
					}
					case QUINCY:
					{
						if (qs.isStarted() && qs.isCond(2))
						{
							htmltext = "33838-01.html";
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
		if ((qs != null) && qs.isCond(1) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			giveItems(player, CHUNK_OF_A_CROPSE, 1);
			playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			if (getQuestItemsCount(player, CHUNK_OF_A_CROPSE) >= 2)
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
	}
}
