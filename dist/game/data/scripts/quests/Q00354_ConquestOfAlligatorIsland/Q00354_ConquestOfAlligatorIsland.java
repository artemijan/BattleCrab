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
package quests.Q00354_ConquestOfAlligatorIsland;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.quest.Quest;
import org.l2jmobius.gameserver.model.quest.QuestState;

/**
 * Conquest of Alligator Island (354)
 * @author Adry_85
 */
public class Q00354_ConquestOfAlligatorIsland extends Quest
{
	// NPC
	private static final int KLUCK = 30895;
	// Items
	private static final int ALLIGATOR_TOOTH = 5863;
	// Misc
	private static final int MIN_LEVEL = 38;
	// Mobs
	private static final Map<Integer, Double> MOB1 = new HashMap<>();
	private static final Map<Integer, Integer> MOB2 = new HashMap<>();
	static
	{
		MOB1.put(20804, 0.84); // crokian_lad
		MOB1.put(20805, 0.91); // dailaon_lad
		MOB1.put(20806, 0.88); // crokian_lad_warrior
		MOB1.put(20807, 0.92); // farhite_lad
		MOB2.put(20808, 14); // nos_lad
	}
	
	public Q00354_ConquestOfAlligatorIsland()
	{
		super(354);
		addStartNpc(KLUCK);
		addTalkId(KLUCK);
		addKillId(MOB1.keySet());
		addKillId(MOB2.keySet());
		registerQuestItems(ALLIGATOR_TOOTH);
		addCondMaxLevel(49, getNoQuestMsg(null));
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
			case "30895-04.html":
			case "30895-05.html":
			case "30895-09.html":
			{
				htmltext = event;
				break;
			}
			case "30895-02.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "ADENA":
			{
				final long count = getQuestItemsCount(player, ALLIGATOR_TOOTH);
				if (count >= 400)
				{
					giveAdena(player, 2000, true);
					takeItems(player, ALLIGATOR_TOOTH, -1);
					htmltext = "30895-06.html";
				}
				else
				{
					htmltext = "30895-08.html";
				}
				break;
			}
			case "30895-10.html":
			{
				qs.exitQuest(true, true);
				htmltext = event;
				break;
			}
		}
		return htmltext;
	}
	
	@Override
	public String onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(player, -1, 3, npc);
		if (qs != null)
		{
			final int npcId = npc.getId();
			if (MOB1.containsKey(npcId))
			{
				giveItemRandomly(qs.getPlayer(), npc, ALLIGATOR_TOOTH, 1, 0, MOB1.get(npcId), true);
			}
			else
			{
				final int itemCount = ((getRandom(100) < MOB2.get(npcId)) ? 2 : 1);
				giveItemRandomly(qs.getPlayer(), npc, ALLIGATOR_TOOTH, itemCount, 0, 1, true);
			}
		}
		return super.onKill(npc, player, isSummon);
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		if (qs.isCreated())
		{
			htmltext = ((player.getLevel() >= MIN_LEVEL) ? "30895-01.htm" : "30895-03.html");
		}
		else if (qs.isStarted())
		{
			htmltext = "30895-04.html";
		}
		return htmltext;
	}
}
