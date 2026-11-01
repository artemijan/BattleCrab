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
package quests.Q10447_TimingIsEverything;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.util.LocationUtil;

import quests.Q10445_AnImpendingThreat.Q10445_AnImpendingThreat;

/**
 * Timing is Everything (10447)
 * @URL https://l2wiki.com/Timing_is_Everything
 * @author Gigi
 */
public class Q10447_TimingIsEverything extends Quest
{
	// Npc
	private static final int BRUENER = 33840;
	
	// Mobs
	private static final int[] MOBS =
	{
		23314, // Nerva Orc Raider
		23315, // Nerva Orc Archer
		23316, // Nerva Orc Priest
		23317, // Nerva Orc Wizard
		23318, // Nerva Orc Assassin
		23319, // Nerva Orc Ambusher
		23320, // Nerva Orc Merchant
		23321, // Nerva Orc Warrior
		23322, // Nerva Orc Prefect
		23323, // Nerva Orc Elite
		23324, // Nerva Bloodlust
		23325, // Nerva Bloodlust
		23326, // Nerva Bloodlust
		23327, // Nerva Bloodlust
		23328, // Nerva Bloodlust
		23329 // Nerva Kaiser
	};
	
	// Item
	private static final int NARVAS_PRISON_KEY = 36665;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	public Q10447_TimingIsEverything()
	{
		super(10447);
		addStartNpc(BRUENER);
		addTalkId(BRUENER);
		addKillId(MOBS);
		registerQuestItems(NARVAS_PRISON_KEY);
		addCondMinLevel(MIN_LEVEL, "33840-00.htm");
		addCondCompletedQuest(Q10445_AnImpendingThreat.class.getSimpleName(), "33840-00.htm");
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
			case "33840-02.htm":
			{
				htmltext = event;
				break;
			}
			case "33840-03.htm":
			{
				qs.startQuest();
				break;
			}
			case "33840-06.html":
			{
				if (qs.isCond(2))
				{
					addExpAndSp(player, 2_147_483_647L, 515396);
					qs.exitQuest(false, true);
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
				htmltext = "33840-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "33840-04.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "33840-05.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getNoQuestMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if (!LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, killer, true))
		{
			return;
		}
		
		if ((qs != null) && qs.isCond(1) && (giveItemRandomly(qs.getPlayer(), npc, NARVAS_PRISON_KEY, 1, 1, 0.1, false)))
		{
			showOnScreenMsg(qs.getPlayer(), NpcStringId.YOU_TOOK_DOWN_THE_NERVA_ORCS_AND_GOT_THEIR_TEMPORARY_PRISON_KEY, ExShowScreenMessage.BOTTOM_RIGHT, 5000);
			qs.setCond(2, true);
		}
		else if (getRandom(100) < 0.03)
		{
			showOnScreenMsg(killer, NpcStringId.YOU_HAVE_OBTAINED_NERVA_S_TEMPORARY_PRISON_KEY, ExShowScreenMessage.BOTTOM_RIGHT, 5000);
			giveItems(killer, NARVAS_PRISON_KEY, 1);
		}
	}
}
