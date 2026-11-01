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
package quests.Q00905_RefinedDragonBlood;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * Refined Dragon Blood (905)
 * @author Zoey76
 */
public class Q00905_RefinedDragonBlood extends Quest
{
	// NPCs
	private static final int[] SEPARATED_SOULS =
	{
		32864,
		32865,
		32866,
		32867,
		32868,
		32869,
		32870,
		32891
	};
	
	// Items
	private static final int UNREFINED_RED_DRAGON_BLOOD = 21913;
	private static final int UNREFINED_BLUE_DRAGON_BLOOD = 21914;
	private static final int REFINED_RED_DRAGON_BLOOD = 21903;
	private static final int REFINED_BLUE_DRAGON_BLOOD = 21904;
	
	// Monsters
	private static final Map<Integer, Integer> MONSTERS = new HashMap<>();
	static
	{
		MONSTERS.put(22844, UNREFINED_BLUE_DRAGON_BLOOD); // Dragon Knight
		MONSTERS.put(22845, UNREFINED_BLUE_DRAGON_BLOOD); // Dragon Knight
		MONSTERS.put(22846, UNREFINED_BLUE_DRAGON_BLOOD); // Elite Dragon Knight
		MONSTERS.put(22847, UNREFINED_RED_DRAGON_BLOOD); // Dragon Knight Warrior
		MONSTERS.put(22848, UNREFINED_RED_DRAGON_BLOOD); // Drake Leader
		MONSTERS.put(22849, UNREFINED_RED_DRAGON_BLOOD); // Drake Warrior
		MONSTERS.put(22850, UNREFINED_RED_DRAGON_BLOOD); // Drake Scout
		MONSTERS.put(22851, UNREFINED_RED_DRAGON_BLOOD); // Drake Mage
		MONSTERS.put(22852, UNREFINED_BLUE_DRAGON_BLOOD); // Dragon Guard
		MONSTERS.put(22853, UNREFINED_BLUE_DRAGON_BLOOD); // Dragon Mage
	}
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final int DRAGON_BLOOD_COUNT = 10;
	
	public Q00905_RefinedDragonBlood()
	{
		super(905);
		addStartNpc(SEPARATED_SOULS);
		addTalkId(SEPARATED_SOULS);
		addKillId(MONSTERS.keySet());
		registerQuestItems(UNREFINED_RED_DRAGON_BLOOD, UNREFINED_BLUE_DRAGON_BLOOD);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			final int itemId = MONSTERS.get(npc.getId());
			if (getQuestItemsCount(player, itemId) < DRAGON_BLOOD_COUNT)
			{
				giveItems(player, itemId, 1);
				if ((getQuestItemsCount(player, UNREFINED_RED_DRAGON_BLOOD) >= DRAGON_BLOOD_COUNT) && (getQuestItemsCount(player, UNREFINED_BLUE_DRAGON_BLOOD) >= DRAGON_BLOOD_COUNT))
				{
					qs.setCond(2, true);
				}
				else
				{
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
			}
		}
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
		if (player.getLevel() >= MIN_LEVEL)
		{
			switch (event)
			{
				case "32864-04.htm":
				case "32864-09.html":
				case "32864-10.html":
				{
					htmltext = event;
					break;
				}
				case "32864-05.htm":
				{
					qs.startQuest();
					htmltext = event;
					break;
				}
				case "32864-11.html":
				{
					giveItems(player, REFINED_RED_DRAGON_BLOOD, 1);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
				case "32864-12.html":
				{
					giveItems(player, REFINED_BLUE_DRAGON_BLOOD, 1);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
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
				htmltext = (player.getLevel() < MIN_LEVEL) ? "32864-02.html" : "32864-01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "32864-06.html";
						break;
					}
					case 2:
					{
						if (!qs.isSet("wait"))
						{
							htmltext = "32864-07.html";
							qs.set("wait", 1);
						}
						else
						{
							htmltext = "32864-08.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = "32864-03.html";
				}
				else
				{
					qs.setState(State.CREATED);
					htmltext = (player.getLevel() < MIN_LEVEL) ? "32864-02.html" : "32864-01.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
}
