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
package quests.Q00310_OnlyWhatRemains;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q00240_ImTheOnlyOneYouCanTrust.Q00240_ImTheOnlyOneYouCanTrust;

/**
 * Only What Remains (310)
 * @author malyelfik
 */
public class Q00310_OnlyWhatRemains extends Quest
{
	// NPC
	private static final int KINTAIJIN = 32640;
	
	// Items
	private static final int GROW_ACCELERATOR = 14832;
	private static final int MULTI_COLORED_JEWEL = 14835;
	private static final int DIRTY_BEAD = 14880;
	
	// Monsters
	private static final Map<Integer, Integer> MOBS = new HashMap<>();
	static
	{
		MOBS.put(22617, 646);
		MOBS.put(22618, 646);
		MOBS.put(22619, 646);
		MOBS.put(22620, 666);
		MOBS.put(22621, 630);
		MOBS.put(22622, 940);
		MOBS.put(22623, 622);
		MOBS.put(22624, 630);
		MOBS.put(22625, 678);
		MOBS.put(22626, 940);
		MOBS.put(22627, 646);
		MOBS.put(22628, 646);
		MOBS.put(22629, 646);
		MOBS.put(22630, 638);
		MOBS.put(22631, 880);
		MOBS.put(22632, 722);
		MOBS.put(22633, 638);
	}
	
	public Q00310_OnlyWhatRemains()
	{
		super(310);
		addStartNpc(KINTAIJIN);
		addTalkId(KINTAIJIN);
		addKillId(MOBS.keySet());
		registerQuestItems(DIRTY_BEAD);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = event;
		switch (event)
		{
			case "32640-04.htm":
			{
				qs.startQuest();
				break;
			}
			case "32640-quit.html":
			{
				qs.exitQuest(true, true);
				break;
			}
			case "32640-02.htm":
			case "32640-03.htm":
			case "32640-05.html":
			case "32640-06.html":
			case "32640-07.html":
			{
				break;
			}
			default:
			{
				htmltext = null;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Player partyMember = getRandomPartyMember(player, 1);
		if (partyMember == null)
		{
			return;
		}
		
		if (getRandom(1000) < MOBS.get(npc.getId()))
		{
			giveItems(partyMember, DIRTY_BEAD, 1);
			playSound(partyMember, QuestSound.ITEMSOUND_QUEST_ITEMGET);
		}
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				final QuestState prev = player.getQuestState(Q00240_ImTheOnlyOneYouCanTrust.class.getSimpleName());
				htmltext = ((player.getLevel() >= 81) && (prev != null) && prev.isCompleted()) ? "32640-01.htm" : "32640-00.htm";
				break;
			}
			case State.STARTED:
			{
				if (!hasQuestItems(player, DIRTY_BEAD))
				{
					htmltext = "32640-08.html";
				}
				else if (getQuestItemsCount(player, DIRTY_BEAD) < 500)
				{
					htmltext = "32640-09.html";
				}
				else
				{
					takeItems(player, DIRTY_BEAD, 500);
					giveItems(player, GROW_ACCELERATOR, 1);
					giveItems(player, MULTI_COLORED_JEWEL, 1);
					htmltext = "32640-10.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
}
