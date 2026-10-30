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
package quests.Q10526_TheDarkSecretOfTheKetraOrcs;

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
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

/**
 * The Dark Secret of the Ketra Orcs (10526)
 * @URL https://l2wiki.com/The_Dark_Secret_of_the_Ketra_Orcs
 * @author Gigi
 * @date 2017-11-20 - [20:03:04]
 */
public class Q10526_TheDarkSecretOfTheKetraOrcs extends Quest
{
	// NPCs
	private static final int LUGONNES = 33852;
	
	// Monsters
	private static final int KETRA_ORC_ELITE_SOLDIER = 21335;
	private static final int KETRA_ORC_CENTURION = 21336;
	private static final int KETRA_ORC_LIEUTENANT = 21332;
	private static final int KETRA_ORC_RAIDER = 21327;
	private static final int KETRA_ORC_WARRIOR = 21331;
	private static final int KETRA_ORC_SCOUT = 21328;
	private static final int KETRA_ORC_OFFICER = 21339;
	private static final int KETRA_ORC_BATTALION_COMMANDER = 21340;
	private static final int KETRA_ORC_HEAD_ROYAL_GUARD = 21346;
	
	private static final int KETRAS_PROPHET = 21347;
	private static final int KETRA_ORC_GRAND_PRIEST = 21342;
	private static final int KETRAS_HEAD_SHAMAN = 21345;
	private static final int KETRA_ORC_SHAMAN = 21329;
	private static final int KETRA_ORC_MEDIUM = 21334;
	
	private static final int KETRA_BACKUP_SHOOTER = 27511;
	private static final int KETRA_BACKUP_WIZARD = 27512;
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 80;
	
	public Q10526_TheDarkSecretOfTheKetraOrcs()
	{
		super(10526);
		addStartNpc(LUGONNES);
		addTalkId(LUGONNES);
		addKillId(KETRA_ORC_HEAD_ROYAL_GUARD, KETRA_ORC_WARRIOR, KETRA_ORC_MEDIUM, KETRA_BACKUP_SHOOTER, KETRA_ORC_SHAMAN, KETRAS_HEAD_SHAMAN, KETRA_BACKUP_WIZARD, KETRA_ORC_ELITE_SOLDIER, KETRA_ORC_CENTURION, KETRA_ORC_LIEUTENANT, KETRA_ORC_RAIDER, KETRAS_PROPHET, KETRA_ORC_SCOUT, KETRA_ORC_OFFICER, KETRA_ORC_BATTALION_COMMANDER, KETRA_ORC_GRAND_PRIEST);
		addCondRace(Race.ERTHEIA, "33852-00a.html");
		addCondStart(p -> p.isInCategory(CategoryType.MAGE_GROUP), "33852-00.htm");
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "33852-00.htm");
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
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 492760460, 5519);
						qs.exitQuest(QuestType.ONE_TIME, true);
						htmltext = event;
					}
					else
					{
						htmltext = getNoQuestLevelRewardMsg(player);
					}
				}
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
			case State.CREATED:
			{
				htmltext = "33852-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "33852-05.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "33852-06.html";
				}
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
		final QuestState qs = getQuestState(killer, true);
		if ((qs != null) && qs.isCond(1))
		{
			int killedShooter = qs.getInt("killed_" + KETRA_BACKUP_SHOOTER);
			int killedWizard = qs.getInt("killed_" + KETRA_BACKUP_WIZARD);
			
			switch (npc.getId())
			{
				case KETRA_ORC_ELITE_SOLDIER:
				case KETRA_ORC_CENTURION:
				case KETRA_ORC_LIEUTENANT:
				case KETRA_ORC_RAIDER:
				case KETRA_ORC_SCOUT:
				case KETRA_ORC_OFFICER:
				case KETRA_ORC_BATTALION_COMMANDER:
				case KETRA_ORC_HEAD_ROYAL_GUARD:
				case KETRA_ORC_WARRIOR:
				{
					final Npc mob = addSpawn(KETRA_BACKUP_SHOOTER, npc, false, 60000);
					mob.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_DARE_INTERFERE_WITH_EMBRYO_SURELY_YOU_WISH_FOR_DEATH);
					addAttackPlayerDesire(mob, killer);
					break;
				}
				
				case KETRAS_PROPHET:
				case KETRA_ORC_GRAND_PRIEST:
				case KETRA_ORC_SHAMAN:
				case KETRAS_HEAD_SHAMAN:
				case KETRA_ORC_MEDIUM:
				{
					final Npc mob = addSpawn(KETRA_BACKUP_WIZARD, npc, false, 60000);
					mob.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_DARE_INTERFERE_WITH_EMBRYO_SURELY_YOU_WISH_FOR_DEATH);
					addAttackPlayerDesire(mob, killer);
					break;
				}
				
				case KETRA_BACKUP_SHOOTER:
				{
					if (killedShooter < 100)
					{
						qs.set("killed_" + KETRA_BACKUP_SHOOTER, ++killedShooter);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case KETRA_BACKUP_WIZARD:
				{
					if (killedWizard < 100)
					{
						qs.set("killed_" + KETRA_BACKUP_WIZARD, ++killedWizard);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
			
			if ((killedShooter >= 100) && (killedWizard >= 100))
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>(2);
			holder.add(new NpcLogListHolder(KETRA_BACKUP_SHOOTER, false, qs.getInt("killed_" + KETRA_BACKUP_SHOOTER)));
			holder.add(new NpcLogListHolder(KETRA_BACKUP_WIZARD, false, qs.getInt("killed_" + KETRA_BACKUP_WIZARD)));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
