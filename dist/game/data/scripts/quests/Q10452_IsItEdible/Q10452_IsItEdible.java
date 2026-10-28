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
package quests.Q10452_IsItEdible;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;

/**
 * Is it Edible? (10452)
 * @URL https://l2wiki.com/Is_it_Edible%3F
 * @author Gigi
 */
public class Q10452_IsItEdible extends Quest
{
	// NPCs
	private static final int SALLY = 32743;
	
	// Monster's
	private static final int FANTASY_MUSHROM = 18864;
	private static final int STICKY_MUSHROMS = 18865;
	private static final int VITALIITY_PLANT = 18868;
	
	// items
	private static final int FANTASY_MUSHROMS_SPORE = 36688;
	private static final int STICKY_MUSHROMS_SPORE = 36689;
	private static final int VITALIITY_LEAF_POUCH = 36690;
	
	// Misc
	private static final int MIN_LEVEL = 81;
	
	public Q10452_IsItEdible()
	{
		super(10452);
		addStartNpc(SALLY);
		addTalkId(SALLY);
		addKillId(FANTASY_MUSHROM, STICKY_MUSHROMS, VITALIITY_PLANT);
		registerQuestItems(FANTASY_MUSHROMS_SPORE, STICKY_MUSHROMS_SPORE, VITALIITY_LEAF_POUCH);
		addCondMinLevel(MIN_LEVEL, "32743-08.htm");
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
			case "32743-02.htm":
			case "32743-09.html":
			case "32743-10.html":
			case "32743-11.html":
			{
				htmltext = event;
				break;
			}
			case "32743-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32743-07.html":
			{
				giveAdena(player, 299940, true);
				addExpAndSp(player, 14120400, 3388);
				qs.exitQuest(false, true);
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
		if (qs.isCreated())
		{
			htmltext = "32743-01.htm";
		}
		else if (qs.isCond(1))
		{
			htmltext = "32743-04.html";
		}
		else if (qs.isCond(2))
		{
			takeItems(player, FANTASY_MUSHROMS_SPORE, -1);
			takeItems(player, STICKY_MUSHROMS_SPORE, -1);
			takeItems(player, VITALIITY_LEAF_POUCH, -1);
			htmltext = "32743-05.html";
			qs.setCond(3);
		}
		else if (qs.isCond(3))
		{
			htmltext = "32743-06.html";
		}
		else
		{
			htmltext = "Complete.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if (qs == null)
		{
			return;
		}
		
		switch (npc.getId())
		{
			case FANTASY_MUSHROM:
			{
				if (qs.isCond(1) && !hasQuestItems(killer, FANTASY_MUSHROMS_SPORE))
				{
					giveItems(killer, FANTASY_MUSHROMS_SPORE, 1);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					break;
				}
			}
			case STICKY_MUSHROMS:
			{
				if (qs.isCond(1) && !hasQuestItems(killer, STICKY_MUSHROMS_SPORE))
				{
					giveItems(killer, STICKY_MUSHROMS_SPORE, 1);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					break;
				}
			}
			case VITALIITY_PLANT:
			{
				if (qs.isCond(1) && !hasQuestItems(killer, VITALIITY_LEAF_POUCH))
				{
					giveItems(killer, VITALIITY_LEAF_POUCH, 1);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					break;
				}
			}
		}
		
		if ((getQuestItemsCount(killer, FANTASY_MUSHROMS_SPORE) >= 1) && (getQuestItemsCount(killer, STICKY_MUSHROMS_SPORE) >= 1) && (getQuestItemsCount(killer, VITALIITY_LEAF_POUCH) >= 1))
		{
			qs.setCond(2, true);
		}
	}
}
