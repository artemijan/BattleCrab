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
package quests.Q10502_FreyaEmbroideredSoulCloak;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * Freya Embroidered Soul Cloak (10502)
 * @author Zoey76, Trevor The Third
 */
public class Q10502_FreyaEmbroideredSoulCloak extends Quest
{
	// NPC
	private static final int OLF_ADAMS = 32612;
	
	// Monster
	private static final int FREYA = 29179;
	private static final int FREYA_HARD = 29180;
	
	// Items
	private static final int FREYAS_SOUL_FRAGMENT = 21723;
	private static final int SOUL_CLOAK_OF_FREYA = 21720;
	
	// Misc
	private static final int MIN_LEVEL = 82;
	private static final int FRAGMENT_COUNT = 20;
	
	public Q10502_FreyaEmbroideredSoulCloak()
	{
		super(10502);
		addStartNpc(OLF_ADAMS);
		addTalkId(OLF_ADAMS);
		addKillId(FREYA, FREYA_HARD);
		registerQuestItems(FREYAS_SOUL_FRAGMENT);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			final long currentCount = getQuestItemsCount(player, FREYAS_SOUL_FRAGMENT);
			final long count = getRandom(1, 3);
			if (count >= (FRAGMENT_COUNT - currentCount))
			{
				giveItems(player, FREYAS_SOUL_FRAGMENT, FRAGMENT_COUNT - currentCount);
				qs.setCond(2, true);
			}
			else
			{
				giveItems(player, FREYAS_SOUL_FRAGMENT, count);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (player.getLevel() >= MIN_LEVEL) && event.equals("32612-04.html"))
		{
			qs.startQuest();
			return event;
		}
		
		return null;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, true);
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = (player.getLevel() < MIN_LEVEL) ? "32612-02.html" : "32612-01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "32612-05.html";
						break;
					}
					case 2:
					{
						if (getQuestItemsCount(player, FREYAS_SOUL_FRAGMENT) >= FRAGMENT_COUNT)
						{
							giveItems(player, SOUL_CLOAK_OF_FREYA, 1);
							playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							qs.exitQuest(false, true);
							htmltext = "32612-06.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "32612-03.html";
				break;
			}
		}
		
		return htmltext;
	}
}
