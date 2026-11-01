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
package events.EveTheFortuneTeller;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.LongTimeEvent;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.enums.LuckyGameType;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;
import org.l2jmobius.gameserver.network.serverpackets.luckygame.ExStartLuckyGame;
import org.l2jmobius.gameserver.util.Broadcast;

/**
 * Eve the Fortune Teller Returns<br>
 * Info - http://www.lineage2.com/en/news/events/11182015-eve-the-fortune-teller-returns.php
 * @author Mobius
 */
public class EveTheFortuneTeller extends LongTimeEvent
{
	// NPCs
	private static final int EVE = 8542;
	private static final int JAYCE = 8540;
	
	// Items
	private static final int FORTUNE_READING_TICKET = 23767;
	private static final int LUXURY_FORTUNE_READING_TICKET = 23768;
	
	// Misc
	private static final NpcStringId[] JAYCE_TEXT =
	{
		NpcStringId.I_LOOK_WEST,
		NpcStringId.EVE_WILL_BRING_YOU_GREAT_FORTUNE,
		NpcStringId.YOU_WILL_ONE_DAY_ASK_ME_FOR_GUIDANCE_IN_YOUR_PATH
	};
	
	private EveTheFortuneTeller()
	{
		addStartNpc(EVE);
		addFirstTalkId(EVE);
		addTalkId(EVE);
		addSpawnId(JAYCE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "8542.htm":
			case "8542-1.htm":
			{
				htmltext = event;
				break;
			}
			case "FortuneReadingGame":
			{
				if (!player.isInventoryUnder80(false))
				{
					player.sendPacket(SystemMessageId.UNABLE_TO_PROCESS_THIS_REQUEST_UNTIL_YOUR_INVENTORY_S_WEIGHT_AND_SLOT_COUNT_ARE_LESS_THAN_80_PERCENT_OF_CAPACITY);
					break;
				}
				
				player.sendPacket(new ExStartLuckyGame(LuckyGameType.NORMAL, player.getInventory().getInventoryItemCount(FORTUNE_READING_TICKET, -1)));
				break;
			}
			case "LuxuryFortuneReadingGame":
			{
				if (!player.isInventoryUnder80(false))
				{
					player.sendPacket(SystemMessageId.UNABLE_TO_PROCESS_THIS_REQUEST_UNTIL_YOUR_INVENTORY_S_WEIGHT_AND_SLOT_COUNT_ARE_LESS_THAN_80_PERCENT_OF_CAPACITY);
					break;
				}
				
				player.sendPacket(new ExStartLuckyGame(LuckyGameType.LUXURY, player.getInventory().getInventoryItemCount(LUXURY_FORTUNE_READING_TICKET, -1)));
				break;
			}
			case "JAYCE_SHOUT":
			{
				Broadcast.toKnownPlayersInRadius(npc, new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), getRandomEntry(JAYCE_TEXT)), 1000);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "8542.htm";
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("JAYCE_SHOUT", 45000, npc, null, true);
	}
	
	public static void main(String[] args)
	{
		new EveTheFortuneTeller();
	}
}
