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
package quests.Q10853_ToWeakenTheGiants;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

/**
 * To Weaken the Giants (10853)
 * @author Kazumi
 */
public final class Q10853_ToWeakenTheGiants extends Quest
{
	// NPCs
	private static final int KRENAHT = 34237;
	private static final int KEKROPUS = 34222;
	
	// Monster
	private static final int MIMILLION_BATHUS = 26138;
	private static final int MIMILLION_CARCASS = 26139;
	private static final int MIMILLION_KSHANA = 26140;
	private static final int MIMIR = 26137;
	private static final int DELTA_BATHUS = 23774;
	private static final int DELTA_CARCASS = 23775;
	private static final int DELTA_KSHANA = 23776;
	
	// Items
	private static final int SPELLBOOK_GRIFFIN = 47151;
	private static final int RUNE_STONE = 39738;
	// private static final int GIANT_TRACKER_MEDAL = 47132;
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10853_ToWeakenTheGiants()
	{
		super(10853);
		addStartNpc(KRENAHT);
		addTalkId(KRENAHT, KEKROPUS);
		addKillId(MIMILLION_BATHUS, MIMILLION_CARCASS, MIMILLION_KSHANA, MIMIR, DELTA_BATHUS, DELTA_CARCASS, DELTA_KSHANA);
		addFactionLevel(Faction.GIANT_TRACKERS, 6, "giantchaser_officer_q10853_03.htm");
		addCondMinLevel(MIN_LEVEL, "giantchaser_officer_q10853_02.htm");
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
			case "giantchaser_officer_q10853_04.htm":
			case "giantchaser_officer_q10853_05.htm":
			{
				htmltext = event;
				break;
			}
			case "giantchaser_officer_q10853_06.htm":
			{
				qs.startQuest();
				break;
			}
			case "leader_kekrops_q10853_02.htm":
			{
				if (qs.isCond(6))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						qs.exitQuest(false, true);
						giveItems(player, SPELLBOOK_GRIFFIN, 1);
						giveItems(player, RUNE_STONE, 25);
						addExpAndSp(player, 3364697600L, 265771800);
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
				if (npc.getId() == KRENAHT)
				{
					if (player.getFactionLevel(Faction.GIANT_TRACKERS) >= 6)
					{
						htmltext = "giantchaser_officer_q10853_01.htm";
						break;
					}
					
					htmltext = "giantchaser_officer_q10853_03.htm";
					break;
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case KRENAHT:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "giantchaser_officer_q10853_07.htm";
								break;
							}
							case 2:
							{
								qs.setCond(3, true);
								htmltext = "giantchaser_officer_q10853_08.htm";
								break;
							}
							case 3:
							{
								htmltext = "giantchaser_officer_q10853_09.htm";
								break;
							}
							case 4:
							{
								htmltext = "giantchaser_officer_q10853_10.htm";
								break;
							}
							case 5:
							{
								qs.setCond(6, true);
								htmltext = "giantchaser_officer_q10853_11.htm";
								break;
							}
							case 6:
							{
								htmltext = "giantchaser_officer_q10853_12.htm";
								break;
							}
						}
						break;
					}
					case KEKROPUS:
					{
						if (qs.isCond(6))
						{
							htmltext = "leader_kekrops_q10853_01.htm";
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
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs != null)
		{
			int npcId = npc.getId();
			switch (npcId)
			{
				case MIMILLION_BATHUS:
				case MIMILLION_CARCASS:
				case MIMILLION_KSHANA:
				{
					if (qs.isCond(1))
					{
						npcId++;
						
						String variable = String.valueOf(npcId); // i3
						int currentValue = qs.getInt(variable);
						if (currentValue < 1)
						{
							qs.set(variable, String.valueOf(currentValue + 1)); // IncreaseNPCLogByID
							
							if ((qs.getInt("26138") == 1) && (qs.getInt("26139") == 1) && (qs.getInt("26140") == 1))
							{
								qs.setCond(2, true);
							}
							else
							{
								playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							}
							
							final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
							log.addNpc(26138, qs.getInt("26138"));
							log.addNpc(26139, qs.getInt("26139"));
							log.addNpc(26140, qs.getInt("26140"));
							player.sendPacket(log);
						}
						break;
					}
					break;
				}
				case MIMIR:
				{
					if (qs.isCond(3))
					{
						qs.setCond(4, true);
					}
					break;
				}
				case DELTA_BATHUS:
				case DELTA_CARCASS:
				case DELTA_KSHANA:
				{
					if (qs.isCond(4))
					{
						npcId++;
						
						String variable = String.valueOf(npcId); // i3
						int currentValue = qs.getInt(variable);
						if (currentValue < 200)
						{
							qs.set(variable, String.valueOf(currentValue + 1)); // IncreaseNPCLogByID
							
							if ((qs.getInt("23774") == 200) && (qs.getInt("23775") == 200) && (qs.getInt("23776") == 200))
							{
								qs.setCond(5, true);
							}
							else
							{
								playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							}
							
							final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
							log.addNpc(23774, qs.getInt("23774"));
							log.addNpc(23775, qs.getInt("23775"));
							log.addNpc(23776, qs.getInt("23776"));
							player.sendPacket(log);
						}
					}
					break;
				}
			}
		}
	}
}
