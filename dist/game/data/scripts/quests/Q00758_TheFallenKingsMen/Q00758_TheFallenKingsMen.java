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
package quests.Q00758_TheFallenKingsMen;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * The Fallen King's Men (758)
 * @URL https://l2wiki.com/The_Fallen_King%27s_Men
 * @author Gigi
 */
public class Q00758_TheFallenKingsMen extends Quest
{
	// Npc
	private static final int INTENDANT = 33407;
	
	// Items
	private static final int TRAVIS_MARK = 36392;
	private static final int REPATRIAT_SOUL = 36393;
	
	// Rewards
	private static final int ESCORT_BOX = 36394;
	
	// Mobs
	private static final int[] MOBS =
	{
		19455, // Aden Raider
		23296, // Chief Quartermaster
		23294, // Chief Magician
		23292, // Royal Guard
		23291, // Personal Magician
		23290, // Royal Knight
		23300, // Commander of Operations
		23299, // Operations Chief of the 7th Division
		23298, // Royal Quartermaster
		23297, // Escort
		23295, // Operations Manager
		23293 // Royal Guard Captain
	};
	private static final int MIN_LEVEL = 97;
	private static final int MARK_COUNT = 100;
	private static final int SOUL_COUNT = 300;
	
	public Q00758_TheFallenKingsMen()
	{
		super(758);
		addStartNpc(INTENDANT);
		addTalkId(INTENDANT);
		registerQuestItems(TRAVIS_MARK, REPATRIAT_SOUL);
		addKillId(MOBS);
		addCondMinLevel(MIN_LEVEL, "33407-00.htm");
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
		
		switch (event)
		{
			case "33407-02.htm":
			case "33407-07.html":
			{
				htmltext = event;
				break;
			}
			case "33407-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33407-06.html":
			{
				if ((getQuestItemsCount(player, TRAVIS_MARK) >= 100) && (getQuestItemsCount(player, REPATRIAT_SOUL) < 100))
				{
					addExpAndSp(player, 3015185490L, 7236360);
					giveItems(player, ESCORT_BOX, 1);
					giveAdena(player, 1017856, false);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, TRAVIS_MARK) >= 100) && ((getQuestItemsCount(player, REPATRIAT_SOUL) >= 100) && (getQuestItemsCount(player, REPATRIAT_SOUL) <= 199)))
				{
					addExpAndSp(player, 6030370980L, 14472720);
					giveItems(player, ESCORT_BOX, 2);
					giveAdena(player, 2035712, false);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, TRAVIS_MARK) >= 100) && ((getQuestItemsCount(player, REPATRIAT_SOUL) >= 200) && (getQuestItemsCount(player, REPATRIAT_SOUL) <= 299)))
				{
					addExpAndSp(player, 9045556470L, 21709080);
					giveItems(player, ESCORT_BOX, 3);
					giveAdena(player, 3053568, false);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				else if ((getQuestItemsCount(player, TRAVIS_MARK) >= 100) && (getQuestItemsCount(player, REPATRIAT_SOUL) >= 300))
				{
					addExpAndSp(player, 12060741960L, 28945440);
					giveItems(player, ESCORT_BOX, 4);
					giveAdena(player, 4071424, false);
					qs.exitQuest(QuestType.DAILY, true);
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
		if (npc.getId() == INTENDANT)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "33407-08.htm";
						break;
					}
					
					qs.setState(State.CREATED);
					// fallthrough
				}
				case State.CREATED:
				{
					htmltext = "33407-01.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1) && (getQuestItemsCount(player, TRAVIS_MARK) < 100))
					{
						htmltext = "33407-04.html";
					}
					else if (qs.isCond(2))
					{
						htmltext = "33407-05.html";
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isStarted())
		{
			if (qs.isCond(1))
			{
				if ((ArrayUtil.contains(MOBS, npc.getId())) && (getQuestItemsCount(killer, TRAVIS_MARK) < MARK_COUNT) && (getRandom(100) < 25))
				{
					giveItems(killer, TRAVIS_MARK, 1);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
				
				if (getQuestItemsCount(killer, TRAVIS_MARK) >= MARK_COUNT)
				{
					qs.setCond(2, true);
				}
			}
			
			if (qs.getCond() > 0)
			{
				if ((ArrayUtil.contains(MOBS, npc.getId())) && (getQuestItemsCount(killer, REPATRIAT_SOUL) < SOUL_COUNT) && (getRandom(100) < 50))
				{
					giveItems(killer, REPATRIAT_SOUL, 1);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
				
				if (getQuestItemsCount(killer, REPATRIAT_SOUL) >= SOUL_COUNT)
				{
					qs.setCond(2, true);
				}
			}
		}
	}
}
