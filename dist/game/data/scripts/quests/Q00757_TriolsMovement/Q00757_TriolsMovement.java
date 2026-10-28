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
package quests.Q00757_TriolsMovement;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Triol's Movement (787)
 * @URL https://l2wiki.com/Triol%27s_Movement
 * @author Gigi
 */
public class Q00757_TriolsMovement extends Quest
{
	// NPC
	private static final int RADZEN = 33803;
	
	// Monsters
	private static final int[] MOBS =
	{
		22139, // Old Aristocrat's Soldier
		22140, // Resurrected Worker
		22141, // Forgotten Victim
		22144, // Resurrected Temple Knight
		22146, // Triol's Priest
		22147, // Ritual Offering
		22152, // Temple Guard
		22153, // Temple Guard Captain
		22154, // Ritual Sacrifice
		22155, // Triol's High Priest
		23278, // Triol's Layperson
		23283 // Triol's Believer
	};
	
	// Items
	private static final int PAGAN_TOTEM = 36230;
	private static final int PAGAN_SOUL = 36231;
	private static final int TOTEM_COUNT = 100;
	private static final int SOUL_COUNT = 300;
	
	// Reward
	private static final int DIVINE_BOX = 36232;
	private static final int ADENA = 57;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q00757_TriolsMovement()
	{
		super(757);
		addStartNpc(RADZEN);
		addTalkId(RADZEN);
		addKillId(MOBS);
		registerQuestItems(PAGAN_TOTEM, PAGAN_SOUL);
		addCondMinLevel(MIN_LEVEL, "no_level.html");
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
			case "start1.htm":
			case "start2.htm":
			case "continue.html":
			case "endquest1.html":
			{
				htmltext = event;
				break;
			}
			case "accepted.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "endquest.html":
			{
				if ((getQuestItemsCount(player, PAGAN_TOTEM) >= 100) && (getQuestItemsCount(player, PAGAN_SOUL) < 100))
				{
					addExpAndSp(player, 3015185490L, 7236360);
					giveItems(player, DIVINE_BOX, 1);
					giveItems(player, ADENA, 745929);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
				}
				else if ((getQuestItemsCount(player, PAGAN_TOTEM) >= 100) && ((getQuestItemsCount(player, PAGAN_SOUL) >= 100) && (getQuestItemsCount(player, PAGAN_SOUL) <= 199)))
				{
					addExpAndSp(player, 6030370980L, 14472720);
					giveItems(player, DIVINE_BOX, 2);
					giveItems(player, ADENA, 1491858);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
				}
				else if ((getQuestItemsCount(player, PAGAN_TOTEM) >= 100) && ((getQuestItemsCount(player, PAGAN_SOUL) >= 200) && (getQuestItemsCount(player, PAGAN_SOUL) <= 299)))
				{
					addExpAndSp(player, 9045556470L, 21709080);
					giveItems(player, DIVINE_BOX, 3);
					giveItems(player, ADENA, 2237787);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
				}
				else if ((getQuestItemsCount(player, PAGAN_TOTEM) >= 100) && (getQuestItemsCount(player, PAGAN_SOUL) >= 300))
				{
					addExpAndSp(player, 12060741960L, 28945440);
					giveItems(player, DIVINE_BOX, 4);
					giveItems(player, ADENA, 2983716);
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
		if (npc.getId() == RADZEN)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "restart.htm";
						break;
					}
					
					qs.setState(State.CREATED);
					// fallthrough
				}
				case State.CREATED:
				{
					htmltext = "start.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "notcollected.html";
					}
					else if (qs.isCond(2))
					{
						htmltext = "collected.html";
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
			if ((ArrayUtil.contains(MOBS, npc.getId())) && (getQuestItemsCount(killer, PAGAN_TOTEM) < TOTEM_COUNT))
			{
				giveItemRandomly(killer, npc, PAGAN_TOTEM, 1, 100, 0.5, true);
			}
			
			if ((ArrayUtil.contains(MOBS, npc.getId())) && (getQuestItemsCount(killer, PAGAN_SOUL) < SOUL_COUNT))
			{
				giveItemRandomly(killer, npc, PAGAN_SOUL, 1, 300, 0.3, true);
			}
			
			if (getQuestItemsCount(killer, PAGAN_TOTEM) >= TOTEM_COUNT)
			{
				qs.setCond(2, true);
			}
		}
	}
}
