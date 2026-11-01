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
package quests.Q10316_UndecayingMemoryOfThePast;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10315_ToThePrisonOfDarkness.Q10315_ToThePrisonOfDarkness;

/**
 * Undecaying Memory of the Past (10316)
 * @URL https://l2wiki.com/Undecaying_Memory_of_the_Past
 * @author Gigi
 */
public class Q10316_UndecayingMemoryOfThePast extends Quest
{
	// NPCs
	private static final int OPERA = 32946;
	private static final int SPEZION = 25779;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	// Item's
	private static final int EAR = 17527;
	private static final int CORRODED_GIANTS_WARSMITH_HOLDER = 19305;
	private static final int CORRODED_GIANTS_REORINS_MOLD = 19306;
	private static final int CORRODED_GIANTS_ARCSMITH_ANVIL = 19307;
	private static final int CORRODED_GIANTS_WARSMITH_MOLD = 19308;
	private static final int HARDENER_POUCHES = 34861;
	
	public Q10316_UndecayingMemoryOfThePast()
	{
		super(10316);
		addStartNpc(OPERA);
		addTalkId(OPERA);
		addKillId(SPEZION);
		addCondMinLevel(MIN_LEVEL, "32946-00.htm");
		addCondCompletedQuest(Q10315_ToThePrisonOfDarkness.class.getSimpleName(), "32946-00a.html");
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
			case "32946-02.htm":
			case "32946-03.htm":
			case "32946-04.htm":
			case "32946-08.html":
			{
				htmltext = event;
				break;
			}
			case "32946-05.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "material":
			{
				giveItems(player, CORRODED_GIANTS_WARSMITH_HOLDER, 1);
				giveItems(player, CORRODED_GIANTS_REORINS_MOLD, 1);
				giveItems(player, CORRODED_GIANTS_ARCSMITH_ANVIL, 1);
				giveItems(player, CORRODED_GIANTS_WARSMITH_MOLD, 1);
				addExpAndSp(player, 54093924, 12982);
				qs.exitQuest(false, true);
				htmltext = "32946-09.html";
				break;
			}
			case "enchant":
			{
				giveItems(player, EAR, 2);
				addExpAndSp(player, 54093924, 12982);
				qs.exitQuest(false, true);
				htmltext = "32946-09.html";
				break;
			}
			case "pouch":
			{
				giveItems(player, HARDENER_POUCHES, 2);
				addExpAndSp(player, 54093924, 12982);
				qs.exitQuest(false, true);
				htmltext = "32946-09.html";
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
				htmltext = "32946-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "32946-06.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "32946-07.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "Complete.html";
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
		if ((qs != null) && qs.isStarted() && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			qs.setCond(2, true);
		}
	}
}
