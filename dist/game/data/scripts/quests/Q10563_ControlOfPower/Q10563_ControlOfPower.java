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
package quests.Q10563_ControlOfPower;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10562_TakeUpArms.Q10562_TakeUpArms;

/**
 * Control of Power (10563)
 * @author Kazumi
 */
public final class Q10563_ControlOfPower extends Quest
{
	// NPCs
	private static final int HERPHAH = 34362;
	private static final int PENNY = 34413;
	
	// Items
	private static final int SOULSHOT_R_GRADE = 33780;
	private static final int B_SPIRITSHOT_R_GRADE = 33794;
	private static final int PA_ART_OF_DESUCTION = 37928;
	private static final int PEARL_LV_1 = 38895;
	private static final int DIAMOND_LV_1 = 38890;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final int MAX_LEVEL = 99;
	
	public Q10563_ControlOfPower()
	{
		super(10563);
		addStartNpc(HERPHAH);
		addTalkId(HERPHAH, PENNY);
		addCondMinLevel(MIN_LEVEL, "herphah_q10563_02.htm");
		addCondMaxLevel(MAX_LEVEL, "");
		addCondCompletedQuest(Q10562_TakeUpArms.class.getSimpleName(), "herphah_q10563_02a.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		if (event.equals("quest_accept"))
		{
			qs.startQuest();
			htmltext = "herphah_q10563_05.htm";
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
				htmltext = "herphah_q10563_01.htm";
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case HERPHAH:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "herphah_q10563_06.htm";
								break;
							}
							case 2:
							{
								htmltext = "herphah_q10563_07.htm";
								break;
							}
							case 3:
							{
								htmltext = "herphah_q10563_08.htm";
								break;
							}
						}
						break;
					}
					case PENNY:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "adventurer_penny_q10563_01.htm";
								break;
							}
							case 2:
							{
								if (player.getFactionLevel(Faction.ADVENTURE_GUILD) < 4)
								{
									htmltext = "adventurer_penny_q10563_03.htm";
									break;
								}
								
								htmltext = "adventurer_penny_q10563_04.htm";
								break;
							}
							case 3:
							{
								htmltext = "adventurer_penny_q10563_06.htm";
								break;
							}
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
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(HERPHAH)
	@Id(PENNY)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10563)
		{
			switch (reply)
			{
				case 1:
				{
					switch (npc.getId())
					{
						case HERPHAH:
						{
							showHtmlFile(player, "herphah_q10563_03.htm");
							break;
						}
						case PENNY:
						{
							qs.setCond(2);
							showHtmlFile(player, "adventurer_penny_q10563_02.htm");
							break;
						}
					}
					break;
				}
				case 2:
				{
					switch (npc.getId())
					{
						case HERPHAH:
						{
							showHtmlFile(player, "herphah_q10563_04.htm");
							break;
						}
						case PENNY:
						{
							qs.setCond(3);
							showHtmlFile(player, "adventurer_penny_q10563_05.htm");
							break;
						}
					}
					break;
				}
				case 9:
				{
					qs.exitQuest(false, true);
					addExpAndSp(player, 11008498436L, 9907649);
					giveItems(player, SOULSHOT_R_GRADE, 10000);
					giveItems(player, B_SPIRITSHOT_R_GRADE, 10000);
					giveItems(player, PA_ART_OF_DESUCTION, 15);
					giveItems(player, PEARL_LV_1, 1);
					giveItems(player, DIAMOND_LV_1, 1);
					showHtmlFile(player, "herphah_q10563_09.htm");
					break;
				}
			}
		}
	}
}
