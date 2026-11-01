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
package quests.Q10832_EnergyOfSadnessAndAnger;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerItemAdd;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10831_UnbelievableSight.Q10831_UnbelievableSight;

/**
 * Energy of Sadness and Anger (10832)
 * @URL https://l2wiki.com/Energy_of_Sadness_and_Anger
 * @author Gigi
 */
public class Q10832_EnergyOfSadnessAndAnger extends Quest
{
	// NPC
	private static final int BELAS = 34056;
	
	// Monsters
	private static final int HARPE = 23561;
	private static final int HARPE1 = 23562;
	private static final int KERBEROS_LAGER_N = 23550; // (night)
	private static final int KERBEROS_LAGER = 23541;
	private static final int KERBEROS_FORT_N = 23551; // (night)
	private static final int KERBEROS_FORT = 23542;
	private static final int KERBEROS_NERO_N = 23552; // (night)
	private static final int KERBEROS_NERO = 23543;
	private static final int FURY_SYLPH_BARRENA_N = 23553; // (night)
	private static final int FURY_SYLPH_BARRENA = 23544;
	private static final int FURY_SYLPH_TEMPTRESS_N = 23555; // (night)
	private static final int FURY_SYLPH_TEMPTRESS = 23546;
	private static final int FURY_SYLPH_PURKA_N = 23556; // (night)
	private static final int FURY_SYLPH_PURKA = 23547;
	private static final int FURY_KERBEROS_LEGER_N = 23557; // (night)
	private static final int FURY_KERBEROS_LEGER = 23545;
	private static final int FURY_KERBEROS_NERO_N = 23558; // (night)
	private static final int FURY_KERBEROS_NERO = 23549;
	
	// Items
	private static final int SAD_ENERGY = 45837;
	private static final int ANGRY_ENERGY = 45838;
	private static final int SOE = 46158;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	public Q10832_EnergyOfSadnessAndAnger()
	{
		super(10832);
		addStartNpc(BELAS);
		addTalkId(BELAS);
		addKillId(HARPE, HARPE1);
		addKillId(KERBEROS_LAGER, KERBEROS_LAGER_N, KERBEROS_FORT, KERBEROS_FORT_N, KERBEROS_NERO, KERBEROS_NERO_N, FURY_SYLPH_BARRENA, FURY_SYLPH_BARRENA_N, FURY_SYLPH_TEMPTRESS, FURY_SYLPH_TEMPTRESS_N, FURY_SYLPH_PURKA, FURY_SYLPH_PURKA_N, FURY_KERBEROS_LEGER, FURY_KERBEROS_LEGER_N, FURY_KERBEROS_NERO, FURY_KERBEROS_NERO_N);
		registerQuestItems(SAD_ENERGY, ANGRY_ENERGY);
		addCondMinLevel(MIN_LEVEL, "34056-00.htm");
		addCondCompletedQuest(Q10831_UnbelievableSight.class.getSimpleName(), "34056-00.htm");
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
			case "34056-02.htm":
			case "34056-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34056-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34056-07.html":
			{
				giveItems(player, SOE, 5);
				addExpAndSp(player, 22221427950L, 22221360);
				qs.exitQuest(false, true);
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
				htmltext = "34056-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34056-05.html";
				}
				else
				{
					htmltext = "34056-06.html";
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
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			switch (npc.getId())
			{
				case KERBEROS_LAGER:
				case KERBEROS_LAGER_N:
				case KERBEROS_FORT:
				case KERBEROS_FORT_N:
				case KERBEROS_NERO:
				case KERBEROS_NERO_N:
				{
					if (getRandom(100) < 2)
					{
						final Npc mob = addSpawn(HARPE, npc.getX(), npc.getY(), npc.getZ(), 0, true, 120000);
						addAttackPlayerDesire(mob, killer, 5);
					}
					break;
				}
				case FURY_SYLPH_BARRENA:
				case FURY_SYLPH_BARRENA_N:
				case FURY_SYLPH_TEMPTRESS:
				case FURY_SYLPH_TEMPTRESS_N:
				case FURY_SYLPH_PURKA:
				case FURY_SYLPH_PURKA_N:
				case FURY_KERBEROS_LEGER:
				case FURY_KERBEROS_LEGER_N:
				case FURY_KERBEROS_NERO:
				case FURY_KERBEROS_NERO_N:
				{
					if (getRandom(100) < 2)
					{
						final Npc mob = addSpawn(HARPE1, npc.getX(), npc.getY(), npc.getZ(), 0, true, 120000);
						addAttackPlayerDesire(mob, killer, 5);
					}
					break;
				}
				case HARPE:
				{
					if ((getRandom(100) < 50) && !hasQuestItems(killer, ANGRY_ENERGY))
					{
						giveItems(killer, ANGRY_ENERGY, 1);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case HARPE1:
				{
					if ((getRandom(100) < 50) && !hasQuestItems(killer, SAD_ENERGY))
					{
						giveItems(killer, SAD_ENERGY, 1);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_ITEM_ADD)
	@RegisterType(ListenerRegisterType.ITEM)
	@Id(ANGRY_ENERGY)
	@Id(SAD_ENERGY)
	public void onItemAdd(OnPlayerItemAdd event)
	{
		final Player player = event.getPlayer();
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.isCond(1)) && (hasQuestItems(player, ANGRY_ENERGY)) && (hasQuestItems(player, SAD_ENERGY)))
		{
			qs.setCond(2, true);
		}
	}
}
