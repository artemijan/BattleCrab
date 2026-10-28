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
package quests.Q10577_TemperARustingBlade;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.Containers;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerAugment;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.Q10566_BestChoice.Q10566_BestChoice;

/**
 * Temper a Rusting Blade (10577)
 * @URL https://l2wiki.com/Temper_a_Rusting_Blade
 * @author NightBR
 * @html by Werum
 */
public class Q10577_TemperARustingBlade extends Quest
{
	// NPCs
	private static final int FLUTTER = 30677;
	
	// Item
	private static final int AUGMENTATION_PRACTICE_STORMBRINGER = 36717;
	private static final int AUGMENTATION_PRACTICE_SPIRIT_STONE = 36718;
	private static final int AUGMENTATION_PRACTICE_GEMSTONE = 36719;
	
	// Rewards
	private static final long XP = 597699960;
	private static final int SP = 597690;
	private static final int CERTIFICATE_FROM_FLUTTER = 48175;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10577_TemperARustingBlade()
	{
		super(10577);
		addStartNpc(FLUTTER);
		addTalkId(FLUTTER);
		addCondMinLevel(MIN_LEVEL, "noLevel.html");
		registerQuestItems(AUGMENTATION_PRACTICE_STORMBRINGER, AUGMENTATION_PRACTICE_SPIRIT_STONE, AUGMENTATION_PRACTICE_GEMSTONE);
		addCondStartedQuest(Q10566_BestChoice.class.getSimpleName(), "30677-99.html");
		Containers.Global().addListener(new ConsumerEventListener(Containers.Global(), EventType.ON_PLAYER_AUGMENT, (OnPlayerAugment event) -> onPlayerAugment(event), this));
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
			case "30677-02.htm":
			case "30677-05.html":
			case "30677-07.html":
			{
				htmltext = event;
				break;
			}
			case "30677-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30677-04.html":
			{
				// show Service/Help/Private Store page
				player.sendPacket(new ExTutorialShowId(57));
				htmltext = event;
				break;
			}
			case "30677-06.html":
			{
				// show Service/Help/Augmentation page
				player.sendPacket(new ExTutorialShowId(39));
				giveItems(player, AUGMENTATION_PRACTICE_STORMBRINGER, 1);
				giveItems(player, AUGMENTATION_PRACTICE_SPIRIT_STONE, 1);
				giveItems(player, AUGMENTATION_PRACTICE_GEMSTONE, 20);
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "30677-08.html":
			{
				addExpAndSp(player, XP, SP);
				giveItems(player, CERTIFICATE_FROM_FLUTTER, 1);
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
				htmltext = "30677-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "30677-03.html";
				}
				else if (qs.isCond(2))
				{
					htmltext = "30677-09.html";
				}
				else
				{
					htmltext = "30677-07.html";
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
	
	public void onPlayerAugment(OnPlayerAugment event)
	{
		final Player player = event.getPlayer();
		if ((player == null) || (event.getItem().getId() != AUGMENTATION_PRACTICE_STORMBRINGER))
		{
			return;
		}
		
		final QuestState qs = getQuestState(player, false);
		
		// Check if weapon has been augmented to complete the quest
		if ((qs != null) && qs.isCond(2) && (player.getInventory().getItemByItemId(AUGMENTATION_PRACTICE_STORMBRINGER).isAugmented()))
		{
			qs.setCond(3, true);
		}
	}
}
