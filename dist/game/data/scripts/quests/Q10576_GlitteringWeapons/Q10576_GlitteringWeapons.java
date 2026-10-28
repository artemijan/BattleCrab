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
package quests.Q10576_GlitteringWeapons;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.Containers;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.holders.item.OnItemEnchantAdd;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.Q10566_BestChoice.Q10566_BestChoice;

/**
 * Glittering Weapons (10576)
 * @URL https://l2wiki.com/Glittering_Weapons
 * @author NightBR
 * @html by Werum
 */
public class Q10576_GlitteringWeapons extends Quest
{
	// NPCs
	private static final int RUPIO = 30471;
	
	// Item
	private static final int ENHANCEMENT_PRACTICE_LONG_SWORD = 48170;
	private static final int PRACTICE_WEAPON_ENCHANT_SCROLL = 48171;
	
	// Rewards
	private static final long XP = 597699960;
	private static final int SP = 597690;
	private static final int CERTIFICATE_FROM_RUPIO = 48174;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10576_GlitteringWeapons()
	{
		super(10576);
		addStartNpc(RUPIO);
		addTalkId(RUPIO);
		addCondMinLevel(MIN_LEVEL, "noLevel.html");
		registerQuestItems(ENHANCEMENT_PRACTICE_LONG_SWORD, PRACTICE_WEAPON_ENCHANT_SCROLL);
		addCondStartedQuest(Q10566_BestChoice.class.getSimpleName(), "30471-99.html");
		Containers.Global().addListener(new ConsumerEventListener(Containers.Global(), EventType.ON_ITEM_ENCHANT_ADD, (OnItemEnchantAdd event) -> onItemEnchantAdd(event), this));
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		String htmltext = null;
		switch (event)
		{
			case "30471-02.htm":
			case "30471-05.html":
			case "30471-07.html":
			case "30471-12.html":
			case "30471-14.html":
			{
				htmltext = event;
				break;
			}
			case "30471-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30471-04.html":
			{
				// show Service/Help/Trade page
				player.sendPacket(new ExTutorialShowId(56));
				htmltext = event;
				break;
			}
			case "30471-06.html":
			{
				// show Service/Help/Enchant page
				player.sendPacket(new ExTutorialShowId(38));
				giveItems(player, ENHANCEMENT_PRACTICE_LONG_SWORD, 1);
				giveItems(player, PRACTICE_WEAPON_ENCHANT_SCROLL, 7);
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "30471-10.html":
			{
				// Rewards
				addExpAndSp(player, XP, SP);
				giveItems(player, CERTIFICATE_FROM_RUPIO, 1);
				qs.exitQuest(QuestType.ONE_TIME, true);
				htmltext = event;
				break;
			}
			case "30471-08.html":
			{
				// check if player already have quest items
				if (hasQuestItems(player, ENHANCEMENT_PRACTICE_LONG_SWORD, PRACTICE_WEAPON_ENCHANT_SCROLL))
				{
					htmltext = "30471-16.html";
				}
				else
				{
					// To make sure player does not have them already
					removeRegisteredQuestItems(player);
					giveItems(player, ENHANCEMENT_PRACTICE_LONG_SWORD, 1);
					giveItems(player, PRACTICE_WEAPON_ENCHANT_SCROLL, 3);
					qs.setCond(3, true);
					htmltext = event;
				}
				break;
			}
			case "30471-11.html":
			{
				// show Service/Help/Trade page
				player.sendPacket(new ExTutorialShowId(56));
				htmltext = event;
				break;
			}
			case "30471-13.html":
			{
				// show Service/Help/Enchant page
				player.sendPacket(new ExTutorialShowId(38));
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
				htmltext = "30471-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "30471-03.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "30471-07.html";
				}
				else if (qs.isCond(3))
				{
					htmltext = "30471-15.html";
				}
				else
				{
					htmltext = "30471-09.html";
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
	
	public void onItemEnchantAdd(OnItemEnchantAdd event)
	{
		final Player player = event.getPlayer();
		if ((player == null) || (event.getItem().getId() != ENHANCEMENT_PRACTICE_LONG_SWORD))
		{
			return;
		}
		
		final QuestState qs = getQuestState(player, false);
		
		// Check if weapon has been augmented to complete the quest
		if ((qs != null) && qs.isCond(2))
		{
			// Check if Item has been destroyed during enchantment process
			if (!hasQuestItems(player, ENHANCEMENT_PRACTICE_LONG_SWORD))
			{
				return;
			}
			
			if (player.getInventory().getItemByItemId(ENHANCEMENT_PRACTICE_LONG_SWORD).getEnchantLevel() == 7)
			{
				qs.setCond(4, true);
			}
		}
		else if ((qs != null) && qs.isCond(3) && (player.getInventory().getItemByItemId(ENHANCEMENT_PRACTICE_LONG_SWORD).getEnchantLevel() == 3))
		{
			qs.setCond(4, true);
		}
	}
}
