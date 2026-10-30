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
package quests.Q10554_GiftForYou;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;
import org.l2jmobius.gameserver.network.serverpackets.ServerPacket;

import quests.Q10553_WhatMattersMoreThanAbility.Q10553_WhatMattersMoreThanAbility;

/**
 * Gift for You (10554)
 * @author Kazumi
 */
public final class Q10554_GiftForYou extends Quest
{
	// NPCs
	private static final int MCCOY = 34383;
	private static final int CAMCUD = 34385;
	private static final int SIBIS = 34384;
	private static final int TARTI = 34359;
	private static final int STIG = 34361;
	
	// ITEMs
	private static final int BK_REWARD_FIGHTER = 48218;
	private static final int BK_REWARD_MYSTIC = 48219;
	private static final int BK_BOX = 48220;
	private static final int BK_SUPPLY_ITEM_LIST = 48221;
	
	public Q10554_GiftForYou()
	{
		super(10554);
		addStartNpc(MCCOY);
		addTalkId(MCCOY, CAMCUD, SIBIS, TARTI, STIG);
		registerQuestItems(BK_SUPPLY_ITEM_LIST, BK_BOX);
		addCondCompletedQuest(Q10553_WhatMattersMoreThanAbility.class.getSimpleName(), "mccoy_q10554_02.htm");
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
			giveItems(player, BK_SUPPLY_ITEM_LIST, 1);
			showOnScreenMsg(player, NpcStringId.GO_FIND_AND_TALK_TO_BALTHUS_KNIGHT_CAMCUD, ExShowScreenMessage.TOP_CENTER, 10000, false);
			htmltext = "mccoy_q10554_05.htm";
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
				switch (npc.getId())
				{
					case MCCOY:
					{
						htmltext = "mccoy_q10554_01.htm";
						break;
					}
					case CAMCUD:
					{
						htmltext = "balthus_camcud_q10554_01.htm";
						break;
					}
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case MCCOY:
					{
						htmltext = "mccoy_q10554_06.htm";
						break;
					}
					case CAMCUD:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "balthus_camcud_q10554_02.htm";
								break;
							}
							case 2:
							{
								htmltext = "balthus_camcud_q10554_05.htm";
								break;
							}
						}
						break;
					}
					case SIBIS:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "sivis_balthus_q10554_01.htm";
								break;
							}
							case 2:
							{
								htmltext = "sivis_balthus_q10554_02.htm";
								break;
							}
							case 3:
							{
								htmltext = "sivis_balthus_q10554_04.htm";
								break;
							}
						}
						break;
					}
					case TARTI:
					{
						switch (qs.getCond())
						{
							case 1:
							case 2:
							{
								htmltext = "tarti_balthus_q10554_01.htm";
								break;
							}
							case 3:
							{
								htmltext = "tarti_balthus_q10554_02.htm";
								break;
							}
						}
						break;
					}
					case STIG:
					{
						switch (qs.getCond())
						{
							case 1:
							case 2:
							case 3:
							{
								htmltext = "stig_q10554_01.htm";
								break;
							}
							case 4:
							{
								htmltext = "stig_q10554_02.htm";
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
	@Id(MCCOY)
	@Id(CAMCUD)
	@Id(SIBIS)
	@Id(TARTI)
	@Id(STIG)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10554)
		{
			switch (reply)
			{
				case 1:
				{
					switch (npc.getId())
					{
						case MCCOY:
						{
							showHtmlFile(player, "mccoy_q10554_03.htm");
							break;
						}
						case CAMCUD:
						{
							final ServerPacket packet = new ExTutorialShowId(56);
							player.sendPacket(packet);
							showHtmlFile(player, "balthus_camcud_q10554_03.htm");
							break;
						}
						case SIBIS:
						{
							final ServerPacket packet = new ExTutorialShowId(12);
							player.sendPacket(packet);
							qs.setCond(3, true);
							showOnScreenMsg(player, NpcStringId.GO_FIND_AND_TALK_TO_TARTI, ExShowScreenMessage.TOP_CENTER, 10000, false);
							takeItems(player, BK_SUPPLY_ITEM_LIST, 1);
							showHtmlFile(player, "sivis_balthus_q10554_03.htm");
							break;
						}
						case TARTI:
						{
							final ServerPacket packet = new ExTutorialShowId(33);
							player.sendPacket(packet);
							qs.setCond(4, true);
							giveItems(player, BK_BOX, 1);
							showOnScreenMsg(player, NpcStringId.TALK_TO_BALTHUS_KNIGHT_CAPTAIN_STIG_MACH, ExShowScreenMessage.TOP_CENTER, 10000, false);
							showHtmlFile(player, "tarti_balthus_q10554_03.htm");
							break;
						}
					}
					break;
				}
				case 2:
				{
					switch (npc.getId())
					{
						case MCCOY:
						{
							showHtmlFile(player, "mccoy_q10554_04.htm");
							break;
						}
						case CAMCUD:
						{
							qs.setCond(2, true);
							showOnScreenMsg(player, NpcStringId.GO_FIND_AND_TALK_TO_BALTHUS_KNIGHT_SIBI, ExShowScreenMessage.TOP_CENTER, 10000, false);
							showHtmlFile(player, "balthus_camcud_q10554_04.htm");
							break;
						}
					}
					break;
				}
				case 10:
				{
					qs.exitQuest(false, true);
					final ServerPacket packet = new ExTutorialShowId(14);
					player.sendPacket(packet);
					if (player.isInCategory(CategoryType.BALTHUS_MAGE_CLASSES))
					{
						showOnScreenMsg(player, NpcStringId.OPEN_YOUR_INVENTORY_AND_DOUBLE_CLICK_THE_BALTHUS_KNIGHT_SUPPLY_BOX_NTO_CHECK_THE_BLESSED_SPIRITSHOTS, ExShowScreenMessage.TOP_CENTER, 10000, false);
						giveItems(player, BK_REWARD_MYSTIC, 1);
					}
					else
					{
						showOnScreenMsg(player, NpcStringId.OPEN_YOUR_INVENTORY_AND_DOUBLE_CLICK_THE_BALTHUS_KNIGHT_SUPPLY_BOX_NTO_CHECK_THE_SOULSHOTS, ExShowScreenMessage.TOP_CENTER, 10000, false);
						giveItems(player, BK_REWARD_FIGHTER, 1);
					}
					
					player.addExpAndSp(643_615_638L, 200_000L);
					showHtmlFile(player, "stig_q10554_03.htm");
					break;
				}
			}
		}
	}
}
