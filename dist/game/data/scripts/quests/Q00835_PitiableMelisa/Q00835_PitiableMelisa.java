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
package quests.Q00835_PitiableMelisa;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.enums.Movie;

/**
 * Pitiable Melisa (835)
 * @URL https://l2wiki.com/Pitiable_Melisa
 * @author Gigi
 * @date 2019-02-04 - [23:59:06]
 */
public class Q00835_PitiableMelisa extends Quest
{
	// NPCs
	private static final int KANNA = 34173;
	private static final int SETTLEN = 34180;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23686, // Frost Glacier Golem
		23687 // Glacier Golem
	};
	
	// Items
	private static final int ICE_CRYSTAL_SHARD = 46594;
	private static final int FRENZED_TAUTIS_FRAGMENT = 47884;
	private static final int INSANE_KELBIMS_FRAGMENT = 47885;
	private static final int SOE_MISTYC_TAVERN = 46564;
	private static final int MYSTIC_ARMOR_PIACE = 46587;
	
	public Q00835_PitiableMelisa()
	{
		super(835);
		addStartNpc(KANNA);
		addTalkId(SETTLEN);
		addKillId(MONSTERS);
		addCreatureSeeId(KANNA);
		registerQuestItems(ICE_CRYSTAL_SHARD);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		switch (event)
		{
			case "NOTIFY_Q835":
			{
				qs.setCond(qs.getCond() + 1, true);
				break;
			}
			case "34180-02.html":
			{
				final int chance = getRandom(100);
				if (chance <= 10)
				{
					giveItems(player, FRENZED_TAUTIS_FRAGMENT, 1);
				}
				else if ((chance > 10) && (chance <= 20))
				{
					giveItems(player, INSANE_KELBIMS_FRAGMENT, 1);
				}
				else if ((chance > 20) && (chance <= 50))
				{
					giveItems(player, MYSTIC_ARMOR_PIACE, 1);
				}
				else
				{
					giveItems(player, SOE_MISTYC_TAVERN, 1);
				}
				
				addExpAndSp(player, 6_362_541_900L, 15_270_101);
				qs.exitQuest(QuestType.REPEATABLE, true);
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.STARTED:
			{
				if (qs.isCond(5))
				{
					htmltext = "34180-01.html";
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		final Player player = creature.asPlayer();
		if (player != null)
		{
			final QuestState qs = getQuestState(player, true);
			if (!qs.isStarted())
			{
				playMovie(player, Movie.EPIC_FREYA_SLIDE);
				qs.startQuest();
			}
		}
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
		if ((qs != null) && qs.isCond(2) && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE) && giveItemRandomly(player, npc, ICE_CRYSTAL_SHARD, 1, 10, 1, true))
		{
			qs.setCond(3, true);
		}
	}
}
