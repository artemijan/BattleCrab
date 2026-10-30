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
package quests.Q10848_TrialsBeforeTheBattle;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Trials before the Battle (10848)
 * @author Kazumi
 */
public final class Q10848_TrialsBeforeTheBattle extends Quest
{
	// NPCs
	private static final int LEONA = 31595;
	private static final int DEVIANNE = 31590;
	private static final int ERICA = 31619;
	private static final int SPORCHA = 34230;
	
	// Monster
	private static final int DARK_WIZARD_OF_MAGIC = 23784;
	private static final int SCORPION_KING = 23785;
	
	// Items
	private static final int PROOFS_OF_BATTLE_READINESS = 47187;
	private static final int SPELLBOOK_HELL_HOUND = 47148;
	private static final int RUNE_STONE = 39738;
	// private static final int BLACKBIRD_CLAN_MEDAL = 47131;
	// Misc
	private static final int MIN_LEVEL = 99;
	private static final int ITEM_COUNT = 500;
	
	public Q10848_TrialsBeforeTheBattle()
	{
		super(10848);
		addStartNpc(LEONA);
		addTalkId(LEONA, DEVIANNE, ERICA, SPORCHA);
		registerQuestItems(PROOFS_OF_BATTLE_READINESS);
		addKillId(DARK_WIZARD_OF_MAGIC, SCORPION_KING);
		addFactionLevel(Faction.BLACKBIRD_CLAN, 6, "lionna_blackbird_q10848_02a.htm");
		addCondMinLevel(MIN_LEVEL, "lionna_blackbird_q10848_02.htm");
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
			case "lionna_blackbird_q10848_04.htm":
			case "lionna_blackbird_q10848_05.htm":
			case "truthseeker_devianne_q10848_02.htm":
			case "erica_ken_weber_q10848_02.htm":
			case "blackbird_sporcha_q10848_02.htm":
			{
				htmltext = event;
				break;
			}
			case "lionna_blackbird_q10848_06.htm":
			{
				qs.startQuest();
				break;
			}
			case "lionna_blackbird_q10848_09.htm":
			{
				qs.setCond(3, true);
				break;
			}
			case "truthseeker_devianne_q10848_03.htm":
			{
				qs.setCond(4, true);
				break;
			}
			case "erica_ken_weber_q10848_03.htm":
			{
				qs.setCond(5, true);
				break;
			}
			case "blackbird_sporcha_q10848_03.htm":
			{
				qs.setCond(6, true);
				break;
			}
			case "lionna_blackbird_q10848_13.htm":
			{
				if (qs.isCond(6))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						qs.exitQuest(false, true);
						giveItems(player, SPELLBOOK_HELL_HOUND, 1);
						giveItems(player, RUNE_STONE, 21);
						addExpAndSp(player, 253076784, 0);
						break;
					}
					
					htmltext = getNoQuestLevelRewardMsg(player);
					break;
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
				if (npc.getId() == LEONA)
				{
					if (player.getFactionLevel(Faction.BLACKBIRD_CLAN) >= 6)
					{
						htmltext = "lionna_blackbird_q10848_01.htm";
						break;
					}
					
					htmltext = "lionna_blackbird_q10848_03.htm";
					break;
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case LEONA:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "lionna_blackbird_q10848_07.htm";
								break;
							}
							case 2:
							{
								htmltext = "lionna_blackbird_q10848_08.htm";
								break;
							}
							case 3:
							{
								htmltext = "lionna_blackbird_q10848_10.htm";
								break;
							}
							case 6:
							{
								htmltext = "lionna_blackbird_q10848_11.htm";
								break;
							}
						}
						break;
					}
					case DEVIANNE:
					{
						switch (qs.getCond())
						{
							case 3:
							{
								htmltext = "truthseeker_devianne_q10848_01.htm";
								break;
							}
							case 4:
							{
								htmltext = "truthseeker_devianne_q10848_04.htm";
								break;
							}
						}
						break;
					}
					case ERICA:
					{
						switch (qs.getCond())
						{
							case 4:
							{
								htmltext = "erica_ken_weber_q10848_01.htm";
								break;
							}
							case 5:
							{
								htmltext = "erica_ken_weber_q10848_04.htm";
								break;
							}
						}
						break;
					}
					case SPORCHA:
					{
						switch (qs.getCond())
						{
							case 5:
							{
								htmltext = "blackbird_sporcha_q10848_01.htm";
								break;
							}
							case 6:
							{
								htmltext = "blackbird_sporcha_q10848_04.htm";
								break;
							}
						}
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
		if ((qs != null) && qs.isCond(1) && giveItemRandomly(qs.getPlayer(), npc, PROOFS_OF_BATTLE_READINESS, 1, ITEM_COUNT, 1.0, true))
		{
			if (getQuestItemsCount(killer, PROOFS_OF_BATTLE_READINESS) >= ITEM_COUNT)
			{
				qs.setCond(2, true);
			}
		}
	}
}
