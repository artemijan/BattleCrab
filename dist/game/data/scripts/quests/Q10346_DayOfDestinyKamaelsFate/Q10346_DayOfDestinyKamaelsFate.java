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
package quests.Q10346_DayOfDestinyKamaelsFate;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerPressTutorialMark;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowHtml;

import quests.ThirdClassTransferQuest;

/**
 * Day of Destiny: Kamael's Fate (10346)
 * @author St3eT, Trevor The Third
 */
public class Q10346_DayOfDestinyKamaelsFate extends ThirdClassTransferQuest
{
	// NPC
	private static final int BROME = 32221;
	
	// Misc
	private static final int QUESTION_MARK_ID = 101;
	private static final int MIN_LEVEL = 76;
	private static final Race START_RACE = Race.KAMAEL;
	
	public Q10346_DayOfDestinyKamaelsFate()
	{
		super(10346, MIN_LEVEL, START_RACE);
		addStartNpc(BROME);
		addTalkId(BROME);
		addCondMinLevel(MIN_LEVEL, "32221-11.html");
		addCondRace(START_RACE, "32221-11.html");
		addCondInCategory(CategoryType.THIRD_CLASS_GROUP, "32221-12.html");
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
			case "32221-02.htm":
			case "32221-03.htm":
			case "32221-04.htm":
			case "32221-08.html":
			{
				htmltext = event;
				break;
			}
			case "32221-05.htm":
			{
				qs.startQuest();
				qs.set("STARTED_CLASS", player.getPlayerClass().getId());
				htmltext = event;
				break;
			}
			default:
			{
				htmltext = super.onEvent(event, npc, player);
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		if (npc.getId() == BROME)
		{
			if (qs.getState() == State.CREATED)
			{
				htmltext = "32221-01.htm";
			}
			else if (qs.getState() == State.STARTED)
			{
				if (qs.isCond(1))
				{
					htmltext = "32221-06.html";
				}
				else if (qs.isCond(13))
				{
					htmltext = "32221-07.html";
				}
			}
		}
		
		return (!htmltext.equals(getNoQuestMsg(player)) ? htmltext : super.onTalk(npc, player));
	}
	
	@RegisterEvent(EventType.ON_PLAYER_PRESS_TUTORIAL_MARK)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerPressTutorialMark(OnPlayerPressTutorialMark event)
	{
		if (event.getMarkId() == QUESTION_MARK_ID)
		{
			final Player player = event.getPlayer();
			if (player.getRace() == START_RACE)
			{
				player.sendPacket(new TutorialShowHtml(getHtm(player, "popupInvite.html")));
			}
		}
	}
}
