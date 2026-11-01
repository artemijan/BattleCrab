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
package ai.areas.GainakUnderground.TavernEmployee;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * Tavern Employee AI.
 * @author Edoo
 */
public class TavernEmployee extends Script
{
	// NPCs
	private static final int LOYEE1 = 34202;
	private static final int LOYEE2 = 34203;
	private static final int LOYEE3 = 34204;
	private static final int LOYEE4 = 34205;
	private static final int LOYEE5 = 34206;
	private static final int LOYEE6 = 34207;
	
	// Text
	private static final NpcStringId[] SPAM_TEXT1 =
	{
		NpcStringId.SIGH_BUSY_AS_ALWAYS,
		NpcStringId.HOW_LONG_UNTIL_WE_CAN_TELL_STORIES_TO_THE_CUSTOMERS_TOO,
		NpcStringId.I_WONDER_WHAT_HANNA_WILL_BE_MAKING_TODAY,
		NpcStringId.HEY_YOU_WE_DON_T_WANT_DRUNK_CUSTOMERS_HERE,
		NpcStringId.WHAT_DO_YOU_THINK_ABOUT_OUR_TAVERN_ISN_T_IT_GREAT,
		NpcStringId.YOU_CAN_PLACE_YOUR_ORDER_OVER_THERE,
		NpcStringId.JUST_RELAX_AND_HAVE_A_DRINK,
		NpcStringId.HANNA_S_COOKING_IS_THE_BEST,
		NpcStringId.YOU_ARE_A_REGULAR_RIGHT_THANKS_FOR_COMING_AGAIN,
		NpcStringId.HERE_YOU_CAN_FORGET_ABOUT_YOUR_RESPONSIBILITIES_FOR_A_WHILE,
		NpcStringId.LUPIA_INTRODUCED_ME_HERE_SO_THAT_S_HOW_I_STARTED_WORKING_HERE,
		NpcStringId.IS_THERE_ANYTHING_TO_CLEAN_UP,
		NpcStringId.I_WONDER_IF_THERE_S_ANYONE_COMING_FROM_THAT_SIDE,
		NpcStringId.I_THINK_WE_CAN_WAIT_FOR_SOME_MORE_CUSTOMERS,
	};
	private static final NpcStringId[] SPAM_TEXT2 =
	{
		NpcStringId.THE_MYSTIC_TAVERN_IS_OPEN_NOW
	};
	private static final NpcStringId[] SPAM_TEXT3 =
	{
		NpcStringId.SIGH_BUSY_AS_ALWAYS,
		NpcStringId.HOW_LONG_UNTIL_WE_CAN_TELL_STORIES_TO_THE_CUSTOMERS_TOO,
		NpcStringId.I_WONDER_WHAT_HANNA_WILL_BE_MAKING_TODAY,
		NpcStringId.HEY_YOU_WE_DON_T_WANT_DRUNK_CUSTOMERS_HERE,
		NpcStringId.WHAT_DO_YOU_THINK_ABOUT_OUR_TAVERN_ISN_T_IT_GREAT,
		NpcStringId.YOU_CAN_PLACE_YOUR_ORDER_OVER_THERE,
		NpcStringId.JUST_RELAX_AND_HAVE_A_DRINK,
		NpcStringId.HANNA_S_COOKING_IS_THE_BEST,
		NpcStringId.YOU_ARE_A_REGULAR_RIGHT_THANKS_FOR_COMING_AGAIN,
		NpcStringId.HERE_YOU_CAN_FORGET_ABOUT_YOUR_RESPONSIBILITIES_FOR_A_WHILE,
		NpcStringId.LUPIA_INTRODUCED_ME_HERE_SO_THAT_S_HOW_I_STARTED_WORKING_HERE,
		NpcStringId.IS_THERE_ANYTHING_TO_CLEAN_UP,
		NpcStringId.I_WONDER_IF_THERE_S_ANYONE_COMING_FROM_THAT_SIDE,
		NpcStringId.I_THINK_WE_CAN_WAIT_FOR_SOME_MORE_CUSTOMERS,
	};
	private static final NpcStringId[] SPAM_TEXT4 =
	{
		NpcStringId.ADVENTURER_THE_TAVERN_IS_THIS_WAY,
		NpcStringId.ARE_YOU_LOOKING_FOR_THE_TAVERN_IT_S_THIS_WAY,
		NpcStringId.COME_ON_CHANCES_LIKE_THESE_DON_T_COME_BY_OFTEN
	};
	private static final NpcStringId[] SPAM_TEXT5 =
	{
		NpcStringId.SIGH_BUSY_AS_ALWAYS,
		NpcStringId.HOW_LONG_UNTIL_WE_CAN_TELL_STORIES_TO_THE_CUSTOMERS_TOO,
		NpcStringId.I_WONDER_WHAT_HANNA_WILL_BE_MAKING_TODAY,
		NpcStringId.HEY_YOU_WE_DON_T_WANT_DRUNK_CUSTOMERS_HERE,
		NpcStringId.WHAT_DO_YOU_THINK_ABOUT_OUR_TAVERN_ISN_T_IT_GREAT,
		NpcStringId.YOU_CAN_PLACE_YOUR_ORDER_OVER_THERE,
		NpcStringId.JUST_RELAX_AND_HAVE_A_DRINK,
		NpcStringId.HANNA_S_COOKING_IS_THE_BEST,
		NpcStringId.YOU_ARE_A_REGULAR_RIGHT_THANKS_FOR_COMING_AGAIN,
		NpcStringId.HERE_YOU_CAN_FORGET_ABOUT_YOUR_RESPONSIBILITIES_FOR_A_WHILE,
		NpcStringId.LUPIA_INTRODUCED_ME_HERE_SO_THAT_S_HOW_I_STARTED_WORKING_HERE,
		NpcStringId.IS_THERE_ANYTHING_TO_CLEAN_UP,
		NpcStringId.I_WONDER_IF_THERE_S_ANYONE_COMING_FROM_THAT_SIDE,
		NpcStringId.I_THINK_WE_CAN_WAIT_FOR_SOME_MORE_CUSTOMERS,
	};
	private static final NpcStringId[] SPAM_TEXT6 =
	{
		NpcStringId.SIGH_BUSY_AS_ALWAYS,
		NpcStringId.HOW_LONG_UNTIL_WE_CAN_TELL_STORIES_TO_THE_CUSTOMERS_TOO,
		NpcStringId.I_WONDER_WHAT_HANNA_WILL_BE_MAKING_TODAY,
		NpcStringId.HEY_YOU_WE_DON_T_WANT_DRUNK_CUSTOMERS_HERE,
		NpcStringId.WHAT_DO_YOU_THINK_ABOUT_OUR_TAVERN_ISN_T_IT_GREAT,
		NpcStringId.YOU_CAN_PLACE_YOUR_ORDER_OVER_THERE,
		NpcStringId.JUST_RELAX_AND_HAVE_A_DRINK,
		NpcStringId.HANNA_S_COOKING_IS_THE_BEST,
		NpcStringId.YOU_ARE_A_REGULAR_RIGHT_THANKS_FOR_COMING_AGAIN,
		NpcStringId.HERE_YOU_CAN_FORGET_ABOUT_YOUR_RESPONSIBILITIES_FOR_A_WHILE,
		NpcStringId.LUPIA_INTRODUCED_ME_HERE_SO_THAT_S_HOW_I_STARTED_WORKING_HERE,
		NpcStringId.IS_THERE_ANYTHING_TO_CLEAN_UP,
		NpcStringId.I_WONDER_IF_THERE_S_ANYONE_COMING_FROM_THAT_SIDE,
		NpcStringId.I_THINK_WE_CAN_WAIT_FOR_SOME_MORE_CUSTOMERS,
	};
	
	private TavernEmployee()
	{
		addSpawnId(LOYEE1);
		addSpawnId(LOYEE2);
		addSpawnId(LOYEE3);
		addSpawnId(LOYEE4);
		addSpawnId(LOYEE5);
		addSpawnId(LOYEE6);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = null;
		switch (event)
		{
			case "spam_text1":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), SPAM_TEXT1[getRandom(SPAM_TEXT1.length)]));
				break;
			}
			case "spam_text2":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), SPAM_TEXT2[getRandom(SPAM_TEXT2.length)]));
				break;
			}
			case "spam_text3":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), SPAM_TEXT3[getRandom(SPAM_TEXT3.length)]));
				break;
			}
			case "spam_text4":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), SPAM_TEXT4[getRandom(SPAM_TEXT4.length)]));
				break;
			}
			case "spam_text5":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), SPAM_TEXT5[getRandom(SPAM_TEXT5.length)]));
				break;
			}
			case "spam_text6":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), SPAM_TEXT6[getRandom(SPAM_TEXT6.length)]));
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.setTalkable(false);
		
		if (npc.getId() == LOYEE1)
		{
			startQuestTimer("spam_text1", 17000, npc, null, true);
		}
		
		if (npc.getId() == LOYEE2)
		{
			startQuestTimer("spam_text2", 180000, npc, null, true);
		}
		
		if (npc.getId() == LOYEE3)
		{
			startQuestTimer("spam_text3", 16000, npc, null, true);
		}
		
		if (npc.getId() == LOYEE4)
		{
			startQuestTimer("spam_text4", 180000, npc, null, true);
		}
		
		if (npc.getId() == LOYEE5)
		{
			startQuestTimer("spam_text5", 15000, npc, null, true);
		}
		
		if (npc.getId() == LOYEE6)
		{
			startQuestTimer("spam_text6", 18000, npc, null, true);
		}
	}
	
	public static void main(String[] args)
	{
		new TavernEmployee();
	}
}
