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
package quests.Q10579_ContainingTheAttributePower;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.Containers;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.holders.item.OnItemAttributeAdd;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.Q10566_BestChoice.Q10566_BestChoice;

/**
 * Containing the Attribute Power (10579)
 * @URL https://l2wiki.com/Containing_the_Attribute_Power
 * @author Werum, NightBR
 */
public class Q10579_ContainingTheAttributePower extends Quest
{
	// NPC
	private static final int FERRIS = 30847;
	
	// Items
	// TODO: Need to add some of the Stones that are not present in the current client
	private static final int ATTRIBUTE_PRACTICE_LONG_SWORD = 48168;
	private static final int ATTRIBUTE_PRACTICE_FIRE_STONE = 48169;
	private static final int ATTRIBUTE_PRACTICE_WATER_STONE = 48169; // FIXME: Does not exist.
	private static final int ATTRIBUTE_PRACTICE_EARTH_STONE = 48169; // FIXME: Does not exist.
	private static final int ATTRIBUTE_PRACTICE_WIND_STONE = 48169; // FIXME: Does not exist.
	private static final int ATTRIBUTE_PRACTICE_HOLY_STONE = 48169; // FIXME: Does not exist.
	private static final int ATTRIBUTE_PRACTICE_DARK_STONE = 48169; // FIXME: Does not exist.
	
	// Rewards
	private static final int XP = 597699960;
	private static final int SP = 597690;
	private static final int CERTIFICATE_FROM_FERRIS = 48177;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10579_ContainingTheAttributePower()
	{
		super(10579);
		addStartNpc(FERRIS);
		addTalkId(FERRIS);
		addCondMinLevel(MIN_LEVEL, "noLevel.html");
		registerQuestItems(ATTRIBUTE_PRACTICE_LONG_SWORD, ATTRIBUTE_PRACTICE_FIRE_STONE, ATTRIBUTE_PRACTICE_WATER_STONE, ATTRIBUTE_PRACTICE_EARTH_STONE, ATTRIBUTE_PRACTICE_WIND_STONE, ATTRIBUTE_PRACTICE_HOLY_STONE, ATTRIBUTE_PRACTICE_DARK_STONE);
		addCondStartedQuest(Q10566_BestChoice.class.getSimpleName(), "30847-99.html");
		Containers.Global().addListener(new ConsumerEventListener(Containers.Global(), EventType.ON_ITEM_ATTRIBUTE_ADD, (OnItemAttributeAdd event) -> onItemAttributeAdd(event), this));
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
			case "30847-02.htm":
			case "30847-05.html":
			case "30847-06.html":
			case "30847-07.html":
			case "30847-13.html":
			case "30847-15.html":
			{
				htmltext = event;
				break;
			}
			case "30847-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30847-04.html":
			{
				// show Service/Help/Auction House page
				player.sendPacket(new ExTutorialShowId(58));
				htmltext = event;
				break;
			}
			case "30847-08.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "30847-12.html":
			{
				// show Service/Help/Auction House page
				player.sendPacket(new ExTutorialShowId(58));
				htmltext = event;
				break;
			}
			case "30847-16.html":
			{
				// show Service/Help/Applying Elemental Attribute page
				player.sendPacket(new ExTutorialShowId(41));
				htmltext = event;
				break;
			}
			case "30847-fire.html":
			{
				// show Service/Help/Applying Elemental Attribute page
				player.sendPacket(new ExTutorialShowId(41));
				giveItems(player, ATTRIBUTE_PRACTICE_LONG_SWORD, 1);
				giveItems(player, ATTRIBUTE_PRACTICE_FIRE_STONE, 3);
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "30847-water.html":
			{
				// show Service/Help/Applying Elemental Attribute page
				player.sendPacket(new ExTutorialShowId(41));
				giveItems(player, ATTRIBUTE_PRACTICE_LONG_SWORD, 1);
				giveItems(player, ATTRIBUTE_PRACTICE_WATER_STONE, 3);
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "30847-earth.html":
			{
				// show Service/Help/Applying Elemental Attribute page
				player.sendPacket(new ExTutorialShowId(41));
				giveItems(player, ATTRIBUTE_PRACTICE_LONG_SWORD, 1);
				giveItems(player, ATTRIBUTE_PRACTICE_EARTH_STONE, 3);
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "30847-wind.html":
			{
				// show Service/Help/Applying Elemental Attribute page
				player.sendPacket(new ExTutorialShowId(41));
				giveItems(player, ATTRIBUTE_PRACTICE_LONG_SWORD, 1);
				giveItems(player, ATTRIBUTE_PRACTICE_WIND_STONE, 3);
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "30847-holy.html":
			{
				// show Service/Help/Applying Elemental Attribute page
				player.sendPacket(new ExTutorialShowId(41));
				giveItems(player, ATTRIBUTE_PRACTICE_LONG_SWORD, 1);
				giveItems(player, ATTRIBUTE_PRACTICE_HOLY_STONE, 3);
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "30847-dark.html":
			{
				// show Service/Help/Applying Elemental Attribute page
				player.sendPacket(new ExTutorialShowId(41));
				giveItems(player, ATTRIBUTE_PRACTICE_LONG_SWORD, 1);
				giveItems(player, ATTRIBUTE_PRACTICE_DARK_STONE, 3);
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "30847-10.html":
			{
				addExpAndSp(player, XP, SP);
				giveItems(player, CERTIFICATE_FROM_FERRIS, 1);
				qs.exitQuest(QuestType.ONE_TIME, true);
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
				htmltext = "30847-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "30847-04.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "30847-08.html";
				}
				else
				{
					htmltext = (qs.isCond(4)) ? "30847-09.html" : "30847-11.html";
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
	
	public void onItemAttributeAdd(OnItemAttributeAdd event)
	{
		final Player player = event.getPlayer();
		if ((player == null) || (event.getItem().getId() != ATTRIBUTE_PRACTICE_LONG_SWORD))
		{
			return;
		}
		
		final QuestState qs = getQuestState(player, false);
		
		// Check weapon has elemental enchant to complete the quest
		if ((qs != null) && qs.isCond(3) && (player.getInventory().getItemByItemId(ATTRIBUTE_PRACTICE_LONG_SWORD).hasAttributes()))
		{
			qs.setCond(4, true);
		}
	}
}
