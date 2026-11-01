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
package quests.Q00181_DevilsStrikeBackAdventOfBalok;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * @author hlwrave
 * @URL https://l2wiki.com/Devils_Strike_Back,_Advent_of_Balok
 */
public class Q00181_DevilsStrikeBackAdventOfBalok extends Quest
{
	// NPC
	private static final int FIOREN = 33044;
	
	// Monster
	private static final int BALOK = 29218;
	
	// Items
	private static final int CONTRACT = 17592;
	private static final int EAR = 17527;
	private static final int EWR = 17526;
	private static final int POUCH = 34861;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q00181_DevilsStrikeBackAdventOfBalok()
	{
		super(181);
		addStartNpc(FIOREN);
		addTalkId(FIOREN);
		addKillId(BALOK);
		registerQuestItems(CONTRACT);
		addCondMinLevel(MIN_LEVEL, "33044-02.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "33044-06.html":
			{
				qs.startQuest();
				break;
			}
			case "reward":
			{
				addExpAndSp(player, 886750000, 414855000);
				giveAdena(player, 37128000, true);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				qs.exitQuest(QuestType.ONE_TIME, true);
				switch (getRandom(3))
				{
					case 0:
					{
						giveItems(player, EWR, 2);
						return "33044-09.html";
					}
					case 1:
					{
						giveItems(player, EAR, 2);
						return "33044-10.html";
					}
					case 2:
					{
						giveItems(player, POUCH, 2);
						return "33044-11.html";
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
		switch (npc.getId())
		{
			case FIOREN:
			{
				if (qs.isCreated())
				{
					htmltext = "33044-01.htm";
				}
				else if (qs.isStarted())
				{
					if (qs.isCond(1))
					{
						htmltext = "33044-07.html";
					}
					else if (qs.isCond(2))
					{
						htmltext = "33044-08.html";
					}
				}
				else if (qs.isCompleted())
				{
					htmltext = "33044-03.html";
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
		if ((qs != null) && qs.isCond(1) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			giveItems(player, CONTRACT, 1);
			playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			qs.setCond(2, true);
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
	}
}
