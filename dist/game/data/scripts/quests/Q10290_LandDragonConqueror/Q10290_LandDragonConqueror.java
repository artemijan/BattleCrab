/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package quests.Q10290_LandDragonConqueror;

import java.util.function.Function;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * Land Dragon Conqueror (10290)
 * @author malyelfik
 */
public class Q10290_LandDragonConqueror extends Quest
{
	// NPC
	private static final int THEODRIC = 30755;
	
	// Monster
	private static final int ANTHARAS = 29068;
	
	// Items
	private static final int PORTAL_STONE = 3865;
	private static final int SHABBY_NECKLACE = 15522;
	private static final int MIRACLE_NECKLACE = 15523;
	
	// Reward
	private static final int ANTHARAS_SLAYER_CIRCLET = 8568;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	
	public Q10290_LandDragonConqueror()
	{
		super(10290);
		addStartNpc(THEODRIC);
		addTalkId(THEODRIC);
		addKillId(ANTHARAS);
		registerQuestItems(MIRACLE_NECKLACE, SHABBY_NECKLACE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		if (event.equals("30755-05.htm"))
		{
			qs.startQuest();
			giveItems(player, SHABBY_NECKLACE, 1);
		}
		
		return event;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		if (!player.isInParty())
		{
			return;
		}
		
		final Function<Player, Boolean> rewardCheck = p ->
		{
			if (LocationUtil.checkIfInRange(8000, npc, p, false))
			{
				final QuestState qs = getQuestState(p, false);
				if ((qs != null) && qs.isCond(1) && hasQuestItems(player, SHABBY_NECKLACE))
				{
					takeItems(player, SHABBY_NECKLACE, -1);
					giveItems(player, MIRACLE_NECKLACE, 1);
					qs.setCond(2, true);
				}
			}
			
			return true;
		};
		
		// rewards go only to command channel, not to a single party or player (retail Freya AI)
		if (player.getParty().isInCommandChannel())
		{
			player.getParty().getCommandChannel().forEachMember(rewardCheck);
		}
		else
		{
			player.getParty().forEachMember(rewardCheck);
		}
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
				if (player.getLevel() < MIN_LEVEL)
				{
					htmltext = "30755-00.htm";
				}
				else
				{
					htmltext = hasQuestItems(player, PORTAL_STONE) ? "30755-02.htm" : "30755-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					if (hasQuestItems(player, SHABBY_NECKLACE))
					{
						htmltext = "30755-06.html";
					}
					else
					{
						giveItems(player, SHABBY_NECKLACE, 1);
						htmltext = "30755-07.html";
					}
				}
				else if (qs.isCond(2) && hasQuestItems(player, MIRACLE_NECKLACE))
				{
					htmltext = "30755-08.html";
					giveAdena(player, 131236, true);
					addExpAndSp(player, 702557, 168);
					giveItems(player, ANTHARAS_SLAYER_CIRCLET, 1);
					qs.exitQuest(false, true);
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "30755-09.html";
				break;
			}
		}
		
		return htmltext;
	}
}
