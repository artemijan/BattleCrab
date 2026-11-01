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
package events.HappyHours;

import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.script.LongTimeEvent;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * @author Mobius
 */
public class HappyHours extends LongTimeEvent
{
	// NPC
	private static final int SIBI = 34262;
	
	// Items
	private static final int SUPPLY_BOX = 47399;
	private static final int SIBIS_COIN = 49783;
	
	// Skill
	private static final int TRANSFORMATION_SKILL = 39171;
	
	// Other
	private static final int MIN_LEVEL = 20;
	private static final int REWARD_INTERVAL = 60 * 60 * 1000; // 1 hour
	private static long _lastRewardTime = System.currentTimeMillis();
	
	private HappyHours()
	{
		addStartNpc(SIBI);
		addFirstTalkId(SIBI);
		addTalkId(SIBI);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "34262-1.htm":
			{
				htmltext = event;
				break;
			}
			case "giveSupplyBox":
			{
				if (player.getLevel() < MIN_LEVEL)
				{
					return "34262-2.htm";
				}
				
				if (ownsAtLeastOneItem(player, SUPPLY_BOX))
				{
					return "34262-3.htm";
				}
				
				giveItems(player, SUPPLY_BOX, 1);
				break;
			}
			case "REWARD_SIBI_COINS":
			{
				if (isEventPeriod())
				{
					if ((System.currentTimeMillis() - (_lastRewardTime + REWARD_INTERVAL)) > 0) // Exploit check - Just in case.
					{
						_lastRewardTime = System.currentTimeMillis();
						final ExShowScreenMessage screenMsg = new ExShowScreenMessage("You obtained 20 Sibi's coins.", ExShowScreenMessage.TOP_CENTER, 7000, 0, true, true);
						final SystemMessage systemMsg = new SystemMessage(SystemMessageId.YOU_HAVE_OBTAINED_S1_SIBI_S_COINS);
						systemMsg.addInt(20);
						for (Player plr : World.getInstance().getPlayers())
						{
							if ((plr != null) && (plr.isOnlineInt() == 1) && plr.isAffectedBySkill(TRANSFORMATION_SKILL))
							{
								plr.addItem(ItemProcessType.REWARD, SIBIS_COIN, 20, player, false);
								plr.sendPacket(screenMsg);
								plr.sendPacket(systemMsg);
								// TODO: Random reward.
							}
						}
					}
				}
				else
				{
					cancelQuestTimers("REWARD_SIBI_COINS");
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "34262.htm";
	}
	
	@Override
	protected void startEvent()
	{
		super.startEvent();
		cancelQuestTimers("REWARD_SIBI_COINS");
		startQuestTimer("REWARD_SIBI_COINS", REWARD_INTERVAL + 1000, null, null, true);
	}
	
	public static void main(String[] args)
	{
		new HappyHours();
	}
}
