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
package quests.Q00571_SpecialMissionProofOfUnityFieldRaid;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

/**
 * Special Mission: Proof of Unity (Field Raid) (571)
 * @URL https://l2wiki.com/Special_Mission:_Proof_of_Unity_(Field_Raid)
 * @author Dmitri
 */
public class Q00571_SpecialMissionProofOfUnityFieldRaid extends Quest
{
	// NPCs
	private static final int PENNY = 34413;
	
	// Raidbosses
	private static final int[] BOSSES =
	{
		25944, // Earth Terakan
		25932, // Transformed: Dartanion
		26011, // Bloody Earth Dragon Gagia
		26012, // Demon Fardune
		26013, // Demon Harsia
		26014, // Demon Bedukel
		26015, // Bloody Witch Rumilla
		26016, // Shilen's Priest Sasia
		26077, // Monster Laum
		26078, // Monster Minotaur
		26079, // Monster Sarga
		26080, // Monster Hogliff
		26081, // Monster Artarot
		26082, // Monster Centaur
	};
	
	// Misc
	private static final int MIN_LEVEL = 86;
	private static final int MAX_LEVEL = 88;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q00571_SpecialMissionProofOfUnityFieldRaid()
	{
		super(571);
		addStartNpc(PENNY);
		addTalkId(PENNY);
		addKillId(BOSSES);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "34413-00.htm");
		addFactionLevel(Faction.ADVENTURE_GUILD, 1, "34413-00.htm");
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
			case "34413-02.htm":
			case "34413-03.htm":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.ADVENTURE_GUILD) >= 1) && (player.getFactionLevel(Faction.ADVENTURE_GUILD) <= 2))
				{
					htmltext = "34413-04.htm";
					break;
				}
				
				htmltext = "34413-00.htm";
				break;
			}
			case "34413-07.html":
			{
				// Rewards
				addExpAndSp(player, 569407440, 569400);
				addFactionPoints(player, Faction.ADVENTURE_GUILD, 220);
				qs.exitQuest(QuestType.DAILY, true);
				htmltext = event;
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
				htmltext = "34413-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.isCond(1)) ? "34413-05.html" : "34413-06.html";
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					qs.setState(State.CREATED);
					htmltext = "34413-01.htm";
				}
				else
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && killer.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			int count = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++count);
			if (count >= 5)
			{
				qs.setCond(2, true);
			}
			else
			{
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				sendNpcLogList(killer);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final int killCount = qs.getInt(KILL_COUNT_VAR);
			if (killCount > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(NpcStringId.LV_86_88_SPECIAL_MISSION_PROOF_OF_UNITY_FIELD_RAID_2.getId(), true, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
