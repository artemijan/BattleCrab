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
package quests.Q10455_ElikiasLetter;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * Elikia's Letter (10455)
 * @URL https://l2wiki.com/Elikia%27s_Letter
 * @author Gigi
 */
public class Q10455_ElikiasLetter extends Quest
{
	// NPCs
	private static final int ELRIKIA_VERDURE_ELDER = 31620;
	private static final int DEVIANNE_TRUTH_SEEKER = 31590;
	private static final int LEONA_BLACKBIRD_FIRE_DRAGON_BRIDE = 31595;
	
	// Items
	private static final int ELRIKIAS_LETTER = 37765;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	public Q10455_ElikiasLetter()
	{
		super(10455);
		addStartNpc(ELRIKIA_VERDURE_ELDER);
		addTalkId(ELRIKIA_VERDURE_ELDER, DEVIANNE_TRUTH_SEEKER, LEONA_BLACKBIRD_FIRE_DRAGON_BRIDE);
		registerQuestItems(ELRIKIAS_LETTER);
		addCondMinLevel(MIN_LEVEL, "31620-00.htm");
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
			case "31620-02.htm":
			case "31620-03.htm":
			case "31595-02.html":
			{
				htmltext = event;
				break;
			}
			case "31620-04.htm":
			{
				qs.startQuest();
				giveItems(player, ELRIKIAS_LETTER, 1);
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, ELRIKIA_VERDURE_ELDER, NpcStringId.YOU_MUST_ACTIVATE_THE_WARP_GATE_BEHIND_ME_IN_ORDER_TO_TELEPORT_TO_HELLBOUND));
				htmltext = event;
				break;
			}
			case "31590-02.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "31595-03.html":
			{
				if (qs.isCond(2))
				{
					giveAdena(player, 32962, true);
					addExpAndSp(player, 3859143, 14816);
					npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, LEONA_BLACKBIRD_FIRE_DRAGON_BRIDE, NpcStringId.HAVE_YOU_MADE_PREPARATIONS_FOR_THE_MISSION_THERE_ISN_T_MUCH_TIME));
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
		switch (npc.getId())
		{
			case ELRIKIA_VERDURE_ELDER:
			{
				if (qs.isCreated())
				{
					htmltext = getHtm(player, "31620-01.htm").replace("%name%", player.getName());
				}
				else if (qs.isCond(1))
				{
					htmltext = "31620-05.html";
				}
				break;
			}
			case DEVIANNE_TRUTH_SEEKER:
			{
				if (qs.isCond(1))
				{
					htmltext = "31590-01.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "31590-03.html";
				}
				break;
			}
			case LEONA_BLACKBIRD_FIRE_DRAGON_BRIDE:
			{
				if (qs.isCond(2))
				{
					htmltext = "31595-01.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
}
