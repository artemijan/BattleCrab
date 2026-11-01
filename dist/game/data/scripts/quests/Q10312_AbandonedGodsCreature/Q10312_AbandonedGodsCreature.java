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
package quests.Q10312_AbandonedGodsCreature;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;

import quests.Q10310_TwistedCreationTree.Q10310_TwistedCreationTree;

/**
 * Abandoned God's Creature (10312)
 * @URL https://l2wiki.com/Abandoned_God%27s_Creature
 * @author Gigi
 */
public class Q10312_AbandonedGodsCreature extends Quest
{
	// Npc
	private static final int HORPINA = 33031;
	
	// Boss
	private static final int APHERUS = 25775;
	
	// Items
	private static final int WARSMITH_HOLDER = 19305; // Corroded Giant's Warsmith' Holder
	private static final int REORINS_MOLD = 19306; // Corroded Giant's Reorin's Mold
	private static final int ARCSMITH_ANVIL = 19307; // Corroded Giant's Arcsmith' Anvil
	private static final int WARSMITH_MOLD = 19308; // Corroded Giant's Warsmith' Mold
	private static final int EAR = 17527; // Scroll: Enchant Armor (R-grade)
	private static final int POUCH = 34861; // Ingredient and Hardener Pouch (R-grade)
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	public Q10312_AbandonedGodsCreature()
	{
		super(10312);
		addStartNpc(HORPINA);
		addTalkId(HORPINA);
		addKillId(APHERUS);
		addCondMinLevel(MIN_LEVEL, "33031-00.htm");
		addCondCompletedQuest(Q10310_TwistedCreationTree.class.getSimpleName(), "33031-00.htm");
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
			case "33031-02.htm":
			case "33031-03.htm":
			case "33031-06.html":
			{
				htmltext = event;
				break;
			}
			case "33031-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "gift1":
			{
				giveItems(player, WARSMITH_HOLDER, 1);
				giveItems(player, REORINS_MOLD, 1);
				giveItems(player, ARCSMITH_ANVIL, 1);
				giveItems(player, WARSMITH_MOLD, 1);
				addExpAndSp(player, 46847289, 11243);
				qs.exitQuest(false, true);
				htmltext = "33031-08.html";
				break;
			}
			case "gift2":
			{
				giveItems(player, EAR, 2);
				addExpAndSp(player, 46847289, 11243);
				qs.exitQuest(false, true);
				htmltext = "33031-08.html";
				break;
			}
			case "gift3":
			{
				giveItems(player, POUCH, 2);
				addExpAndSp(player, 46847289, 11243);
				qs.exitQuest(false, true);
				htmltext = "33031-08.html";
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
			htmltext = "33031-01.htm";
		}
		else if (qs.isCond(1))
		{
			htmltext = "33031-05.html";
		}
		else if (qs.isCond(2))
		{
			htmltext = "33031-07.html";
		}
		else if (qs.isCompleted())
		{
			htmltext = "Complete.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			qs.setCond(2, true);
		}
	}
}
