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
package quests.Q10425_TheKetraOrcSupporters;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * The Ketra Orc Supporters (10425)
 * @author Stayway
 */
public class Q10425_TheKetraOrcSupporters extends Quest
{
	// NPCs
	private static final int LUGONNES = 33852;
	private static final int EMBRYO_SHOOTER = 27511;
	private static final int EMBRYO_WIZARD = 27512;
	private static final int[] SHOOTER_MONSTERS =
	{
		21327, // Ketra Orc Raider
		21331, // Ketra Orc Warrior
		21332, // Ketra Orc Lieutenant
		21335, // Ketra Orc Elite Soldier
		21336, // Ketra Orc White Captain
		21339, // Ketra Orc Officer
		21340, // Ketra Orc Battalion Commander
		27511, // Ketra Backup Shooter
	};
	private static final int[] WIZARD_MONSTERS =
	{
		21334, // Ketra Orc Medium
		21338, // Ketra Orc Seer
		21342, // Ketra Orc Grand Priest
		27512, // Varka Backup Wizard
	};
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 80;
	
	public Q10425_TheKetraOrcSupporters()
	{
		super(10425);
		addStartNpc(LUGONNES);
		addTalkId(LUGONNES);
		addKillId(SHOOTER_MONSTERS);
		addKillId(WIZARD_MONSTERS);
		addCondNotRace(Race.ERTHEIA, "33852-09.html");
		addCondInCategory(CategoryType.WIZARD_GROUP, "33852-08.html");
		addCondMinLevel(MIN_LEVEL, "33852-08.html");
		addCondMaxLevel(MAX_LEVEL, "33852-08.html");
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
			case "33852-02.htm":
			case "33852-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33852-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33852-07.html":
			{
				if (qs.isCond(2))
				{
					qs.exitQuest(false, true);
					giveStoryQuestReward(npc, player);
					if ((player.getLevel() >= MIN_LEVEL) && (player.getLevel() <= MAX_LEVEL))
					{
						addExpAndSp(player, 492760460, 5519);
					}
					
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
		String htmltext = null;
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "33852-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = qs.isCond(1) ? "33852-05.html" : "33852-06.html";
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			if ((npc.getId() == EMBRYO_SHOOTER) || (npc.getId() == EMBRYO_WIZARD))
			{
				int shooterCount = qs.getInt("KillCount_" + EMBRYO_SHOOTER);
				int wizardCount = qs.getInt("KillCount_" + EMBRYO_WIZARD);
				if (npc.getId() == EMBRYO_SHOOTER)
				{
					if (shooterCount < 100)
					{
						qs.set("KillCount_" + EMBRYO_SHOOTER, ++shooterCount);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
				}
				else if (wizardCount < 100)
				{
					qs.set("KillCount_" + EMBRYO_WIZARD, ++wizardCount);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
				
				if ((shooterCount >= 100) && (wizardCount >= 100))
				{
					qs.setCond(2, true);
				}
			}
			else if (ArrayUtil.contains(WIZARD_MONSTERS, npc.getId()))
			{
				if (qs.getInt("KillCount_" + EMBRYO_WIZARD) < 100)
				{
					final Npc embryo = addSpawn(EMBRYO_WIZARD, npc, false, 60000);
					addAttackPlayerDesire(embryo, killer);
					embryo.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_DARE_INTERFERE_WITH_EMBRYO_SURELY_YOU_WISH_FOR_DEATH);
				}
			}
			else if (qs.getInt("KillCount_" + EMBRYO_SHOOTER) < 100)
			{
				final Npc embryo = addSpawn(EMBRYO_SHOOTER, npc, false, 60000);
				addAttackPlayerDesire(embryo, killer);
				embryo.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_DARE_INTERFERE_WITH_EMBRYO_SURELY_YOU_WISH_FOR_DEATH);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final Set<NpcLogListHolder> npcLogList = new HashSet<>(2);
			npcLogList.add(new NpcLogListHolder(EMBRYO_SHOOTER, false, qs.getInt("KillCount_" + EMBRYO_SHOOTER)));
			npcLogList.add(new NpcLogListHolder(EMBRYO_WIZARD, false, qs.getInt("KillCount_" + EMBRYO_WIZARD)));
			return npcLogList;
		}
		
		return super.getNpcLogList(player);
	}
}
