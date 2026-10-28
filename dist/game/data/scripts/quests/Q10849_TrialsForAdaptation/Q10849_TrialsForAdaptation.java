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
package quests.Q10849_TrialsForAdaptation;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Trials for Adaptation (10849)
 * @URL https://l2wiki.com/Trials_for_Adaptation
 * @author Dmitri
 */
public class Q10849_TrialsForAdaptation extends Quest
{
	// NPC
	private static final int FERIN = 34054;
	private static final int EUBINA = 34238;
	private static final int ROTOEH = 34239;
	private static final int CYPHONA = 34055;
	private static final int BELAS = 34056;
	
	// Monsters
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
	private static final int ENERGY_LADEL_WITH_THE_DAYS = 47189;
	private static final int ENERGY_LADEL_WITH_THE_NIGHTS = 47190;
	
	// Reward
	private static final int RUNE_STONE = 39738;
	private static final int SPELLBOOK_WING_HOUND = 47152;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	public Q10849_TrialsForAdaptation()
	{
		super(10849);
		addStartNpc(FERIN);
		addTalkId(FERIN, EUBINA, ROTOEH, CYPHONA, BELAS);
		addKillId(KERBEROS_LAGER, KERBEROS_LAGER_N, KERBEROS_FORT, KERBEROS_FORT_N, KERBEROS_NERO, KERBEROS_NERO_N, FURY_SYLPH_BARRENA, FURY_SYLPH_BARRENA_N, FURY_SYLPH_TEMPTRESS, FURY_SYLPH_TEMPTRESS_N, FURY_SYLPH_PURKA, FURY_SYLPH_PURKA_N, FURY_KERBEROS_LEGER, FURY_KERBEROS_LEGER_N, FURY_KERBEROS_NERO, FURY_KERBEROS_NERO_N);
		registerQuestItems(ENERGY_LADEL_WITH_THE_DAYS, ENERGY_LADEL_WITH_THE_NIGHTS);
		addCondMinLevel(MIN_LEVEL, "34054-00.htm");
		addFactionLevel(Faction.UNWORLDLY_VISITORS, 6, "34054-00.htm");
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
			case "34054-02.htm":
			case "34054-03.htm":
			case "34238-03.html":
			case "34239-03.html":
			case "34055-03.html":
			case "34056-03.html":
			{
				htmltext = event;
				break;
			}
			case "34054-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34054-07.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34238-02.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34239-02.html":
			{
				qs.setCond(5, true);
				htmltext = event;
				break;
			}
			case "34055-02.html":
			{
				qs.setCond(6, true);
				htmltext = event;
				break;
			}
			case "34056-02.html":
			{
				qs.setCond(7, true);
				htmltext = event;
				break;
			}
			case "34054-10.html":
			{
				if (qs.isCond(7))
				{
					giveItems(player, RUNE_STONE, 1);
					giveItems(player, SPELLBOOK_WING_HOUND, 1);
					addExpAndSp(player, 444428559000L, 444427200);
					qs.exitQuest(false, true);
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
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npc.getId() == FERIN)
				{
					htmltext = "34054-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case FERIN:
					{
						if (qs.isCond(1))
						{
							htmltext = "34054-05.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "34054-06.html";
						}
						else if (qs.isCond(3))
						{
							htmltext = "34054-07.html";
						}
						else if (qs.isCond(7))
						{
							htmltext = "34054-09.html";
						}
						break;
					}
					case EUBINA:
					{
						if (qs.isCond(3))
						{
							htmltext = "34238-01.html";
						}
						else if (qs.isCond(4))
						{
							htmltext = "34238-03.html";
						}
						break;
					}
					case ROTOEH:
					{
						if (qs.isCond(4))
						{
							htmltext = "34239-01.html";
						}
						else if (qs.isCond(5))
						{
							htmltext = "34239-03.html";
						}
						break;
					}
					case CYPHONA:
					{
						if (qs.isCond(5))
						{
							htmltext = "34055-01.html";
						}
						else if (qs.isCond(6))
						{
							htmltext = "34055-03.html";
						}
						break;
					}
					case BELAS:
					{
						if (qs.isCond(6))
						{
							htmltext = "34056-01.html";
						}
						else if (qs.isCond(7))
						{
							htmltext = "34056-03.html";
						}
						break;
					}
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
		if ((qs != null) && qs.isCond(1) && killer.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			switch (npc.getId())
			{
				case FURY_SYLPH_BARRENA_N:
				case FURY_SYLPH_TEMPTRESS_N:
				case FURY_SYLPH_PURKA_N:
				case FURY_KERBEROS_LEGER_N:
				case FURY_KERBEROS_NERO_N:
				case KERBEROS_LAGER_N:
				case KERBEROS_FORT_N:
				case KERBEROS_NERO_N:
				{
					if (getRandom(100) < 99)
					{
						giveItems(killer, ENERGY_LADEL_WITH_THE_NIGHTS, 1);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case FURY_SYLPH_BARRENA:
				case FURY_SYLPH_TEMPTRESS:
				case FURY_SYLPH_PURKA:
				case FURY_KERBEROS_LEGER:
				case FURY_KERBEROS_NERO:
				case KERBEROS_LAGER:
				case KERBEROS_FORT:
				case KERBEROS_NERO:
				{
					if (getRandom(100) < 99)
					{
						giveItems(killer, ENERGY_LADEL_WITH_THE_DAYS, 1);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
			
			if ((getQuestItemsCount(killer, ENERGY_LADEL_WITH_THE_DAYS) >= 1500) && (getQuestItemsCount(killer, ENERGY_LADEL_WITH_THE_NIGHTS) >= 500))
			{
				qs.setCond(2, true);
			}
		}
	}
}
