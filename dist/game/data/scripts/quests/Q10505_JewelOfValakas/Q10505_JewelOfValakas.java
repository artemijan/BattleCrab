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
package quests.Q10505_JewelOfValakas;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * Jewel of Valakas (10505)
 * @author Zoey76
 */
public class Q10505_JewelOfValakas extends Quest
{
	// NPC
	private static final int KLEIN = 31540;
	
	// Monster
	private static final int VALAKAS = 29028;
	
	// Items
	private static final int EMPTY_CRYSTAL = 21906;
	private static final int FILLED_CRYSTAL_VALAKAS_ENERGY = 21908;
	private static final int JEWEL_OF_VALAKAS = 21896;
	private static final int VACUALITE_FLOATING_STONE = 7267;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	
	public Q10505_JewelOfValakas()
	{
		super(10505);
		addStartNpc(KLEIN);
		addTalkId(KLEIN);
		addKillId(VALAKAS);
		registerQuestItems(EMPTY_CRYSTAL, FILLED_CRYSTAL_VALAKAS_ENERGY);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			takeItems(player, EMPTY_CRYSTAL, -1);
			giveItems(player, FILLED_CRYSTAL_VALAKAS_ENERGY, 1);
			playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			qs.setCond(2, true);
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
		if ((player.getLevel() >= MIN_LEVEL) && hasQuestItems(player, VACUALITE_FLOATING_STONE))
		{
			switch (event)
			{
				case "31540-05.htm":
				case "31540-06.htm":
				{
					htmltext = event;
					break;
				}
				case "31540-07.html":
				{
					qs.startQuest();
					giveItems(player, EMPTY_CRYSTAL, 1);
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
		executeForEachPlayer(killer, npc, isSummon, true, true);
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
					htmltext = "31540-02.html";
				}
				else if (!hasQuestItems(player, VACUALITE_FLOATING_STONE))
				{
					htmltext = "31540-04.html";
				}
				else
				{
					htmltext = "31540-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						if (hasQuestItems(player, EMPTY_CRYSTAL))
						{
							htmltext = "31540-08.html";
						}
						else
						{
							giveItems(player, EMPTY_CRYSTAL, 1);
							htmltext = "31540-09.html";
						}
						break;
					}
					case 2:
					{
						giveItems(player, JEWEL_OF_VALAKAS, 1);
						playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						qs.exitQuest(false, true);
						htmltext = "31540-10.html";
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "31540-03.html";
				break;
			}
		}
		
		return htmltext;
	}
}
