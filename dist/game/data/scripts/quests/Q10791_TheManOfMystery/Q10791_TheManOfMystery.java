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
package quests.Q10791_TheManOfMystery;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.actor.instance.FriendlyNpc;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

import quests.Q10790_AMercenaryHelper.Q10790_AMercenaryHelper;

/**
 * The Man Of Mystery (10791)
 * @URL https://l2wiki.com/The_Man_of_Mystery
 * @author Gigi
 */
public class Q10791_TheManOfMystery extends Quest
{
	// NPCs
	private static final int DOKARA = 33847;
	private static final int KAIN_VAN_HALTER = 33993;
	
	// Monsters
	private static final int SUSPICIOUS_COCOON = 27536;
	private static final int SUSPICIOUS_COCOON1 = 27537;
	private static final int SUSPICIOUS_COCOON2 = 27538;
	private static final int NEEDLE_STAKATO_CAPTAIN = 27542;
	private static final int NEEDLE_STAKATO = 27543;
	
	// Misc
	private static final int MIN_LEVEL = 65;
	private static final int MAX_LEVEL = 70;
	
	public Q10791_TheManOfMystery()
	{
		super(10791);
		addStartNpc(DOKARA);
		addTalkId(DOKARA, KAIN_VAN_HALTER);
		addFirstTalkId(KAIN_VAN_HALTER);
		addKillId(SUSPICIOUS_COCOON, SUSPICIOUS_COCOON1, SUSPICIOUS_COCOON2, NEEDLE_STAKATO_CAPTAIN);
		addAttackId(NEEDLE_STAKATO_CAPTAIN);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "no_level.html");
		addCondRace(Race.ERTHEIA, "noErtheia.html");
		addCondClassId(PlayerClass.MARAUDER, "no_quest.html");
		addCondCompletedQuest(Q10790_AMercenaryHelper.class.getSimpleName(), "restriction.html");
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
			case "33847-02.htm":
			case "33847-03.htm":
			{
				htmltext = event;
				break;
			}
			case "33847-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "thank":
			{
				npc.deleteMe();
				htmltext = "33993-01.html";
				break;
			}
			default:
			{
				if (qs.isCond(3))
				{
					addExpAndSp(player, 46334481, 4072);
					giveStoryQuestReward(npc, player);
					qs.exitQuest(false, true);
					htmltext = "33847-07.html";
					break;
				}
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
				htmltext = "33847-01.htm";
				break;
			}
			case State.STARTED:
			{
				if ((qs.getCond() > 0) && (qs.getCond() < 3))
				{
					htmltext = "33847-05.html";
				}
				else if (qs.isCond(3))
				{
					htmltext = "33847-06.html";
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
	public String onFirstTalk(Npc npc, Player player)
	{
		return "33993.html";
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if (npc.isScriptValue(0))
		{
			for (int i = 0; i < 5; i++)
			{
				final Npc creature = addSpawn(NEEDLE_STAKATO, npc.getX() + getRandom(-20, 20), npc.getY() + getRandom(-20, 20), npc.getZ(), npc.getHeading(), true, 120000, false);
				addAttackPlayerDesire(creature, attacker);
				npc.setScriptValue(1);
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, true);
		if ((qs != null) && (qs.getCond() > 0))
		{
			switch (npc.getId())
			{
				case SUSPICIOUS_COCOON:
				case SUSPICIOUS_COCOON1:
				case SUSPICIOUS_COCOON2:
				{
					int kills = qs.getInt(Integer.toString(SUSPICIOUS_COCOON));
					if (kills < 5)
					{
						kills++;
						qs.set(Integer.toString(SUSPICIOUS_COCOON), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					
					if (kills >= 5)
					{
						final Npc monster = addSpawn(NEEDLE_STAKATO_CAPTAIN, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), true, 600000, false);
						final FriendlyNpc kain = (FriendlyNpc) addSpawn(KAIN_VAN_HALTER, killer.getX() + getRandom(-100, 100), killer.getY() + getRandom(-100, 100), killer.getZ(), 0, true, 300000, false);
						kain.setRunning();
						kain.setInvul(true);
						kain.reduceCurrentHp(1, monster, null); // TODO: Find better way for attack
						addAttackPlayerDesire(monster, killer);
						qs.setCond(2);
					}
					break;
				}
				case NEEDLE_STAKATO_CAPTAIN:
				{
					int kills = qs.getInt(Integer.toString(NEEDLE_STAKATO_CAPTAIN));
					if ((kills < 1) && qs.isCond(2))
					{
						kills++;
						qs.set(Integer.toString(NEEDLE_STAKATO_CAPTAIN), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					
					if (qs.getInt(Integer.toString(NEEDLE_STAKATO_CAPTAIN)) >= 1)
					{
						qs.setCond(1);
						qs.setCond(3, true);
					}
					break;
				}
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpc(SUSPICIOUS_COCOON, qs.getInt(Integer.toString(SUSPICIOUS_COCOON)));
			log.addNpc(NEEDLE_STAKATO_CAPTAIN, qs.getInt(Integer.toString(NEEDLE_STAKATO_CAPTAIN)));
			qs.getPlayer().sendPacket(log);
		}
	}
}
