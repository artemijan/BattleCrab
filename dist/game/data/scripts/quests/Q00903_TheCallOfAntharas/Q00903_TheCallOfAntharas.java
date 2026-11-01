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
package quests.Q00903_TheCallOfAntharas;

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
 * The Call of Antharas (903)
 * @author Zoey76
 */
public class Q00903_TheCallOfAntharas extends Quest
{
	// NPC
	private static final int THEODRIC = 30755;
	
	// Monsters
	private static final int BEHEMOTH_DRAGON = 29069;
	private static final int TARASK_DRAGON = 29190;
	
	// Items
	private static final int TARASK_DRAGONS_LEATHER_FRAGMENT = 21991;
	private static final int BEHEMOTH_DRAGON_LEATHER = 21992;
	private static final int SCROLL_ANTHARAS_CALL = 21897;
	private static final int PORTAL_STONE = 3865;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	
	public Q00903_TheCallOfAntharas()
	{
		super(903);
		addStartNpc(THEODRIC);
		addTalkId(THEODRIC);
		addKillId(BEHEMOTH_DRAGON, TARASK_DRAGON);
		registerQuestItems(TARASK_DRAGONS_LEATHER_FRAGMENT, BEHEMOTH_DRAGON_LEATHER);
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
		if ((player.getLevel() >= MIN_LEVEL) && hasQuestItems(player, PORTAL_STONE))
		{
			switch (event)
			{
				case "30755-05.htm":
				{
					htmltext = event;
					break;
				}
				case "30755-06.html":
				{
					qs.startQuest();
					htmltext = event;
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			switch (npc.getId())
			{
				case BEHEMOTH_DRAGON:
				{
					giveItems(player, BEHEMOTH_DRAGON_LEATHER, 1);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					break;
				}
				case TARASK_DRAGON:
				{
					giveItems(player, TARASK_DRAGONS_LEATHER_FRAGMENT, 1);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					break;
				}
			}
			
			if (hasQuestItems(player, BEHEMOTH_DRAGON_LEATHER) && hasQuestItems(player, TARASK_DRAGONS_LEATHER_FRAGMENT))
			{
				qs.setCond(2, true);
			}
		}
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
				if (player.getLevel() < MIN_LEVEL)
				{
					htmltext = "30755-03.html";
				}
				else if (!hasQuestItems(player, PORTAL_STONE))
				{
					htmltext = "30755-04.html";
				}
				else
				{
					htmltext = "30755-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "30755-07.html";
						break;
					}
					case 2:
					{
						giveItems(player, SCROLL_ANTHARAS_CALL, 1);
						playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						qs.exitQuest(QuestType.DAILY, true);
						htmltext = "30755-08.html";
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = "30755-02.html";
				}
				else
				{
					qs.setState(State.CREATED);
					if (player.getLevel() < MIN_LEVEL)
					{
						htmltext = "30755-03.html";
					}
					else if (!hasQuestItems(player, PORTAL_STONE))
					{
						htmltext = "30755-04.html";
					}
					else
					{
						htmltext = "30755-01.htm";
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
}
