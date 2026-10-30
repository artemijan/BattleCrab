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
package quests.Q00826_InSearchOfTheSecretWeapon;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * In Search of the Secret Weapon (826)
 * @URL https://l2wiki.com/In_Search_of_the_Secret_Weapon
 * @author Mobius, Liamxroy
 */
public class Q00826_InSearchOfTheSecretWeapon extends Quest
{
	// NPC
	private static final int NETI = 34095;
	private static final int[] COMMANDERS =
	{
		23653, // Unit Commander 1
		23654, // Unit Commander 2
		23655, // Unit Commander 2
		23656, // Unit Commander 2
		23657, // Unit Commander 3
		23658, // Unit Commander 4
		23659, // Unit Commander 4
		23660, // Unit Commander 5
		23661, // Unit Commander 6
		23662, // Unit Commander 7
		23663, // Unit Commander 8
		23664, // Unit Commander 8
	};
	
	// Items
	private static final int ASHEN_CERTIFICATE = 46371;
	private static final int SHADOW_WEAPON_COUPON = 46376;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q00826_InSearchOfTheSecretWeapon()
	{
		super(826);
		addStartNpc(NETI);
		addTalkId(NETI);
		addKillId(COMMANDERS);
		addCondMinLevel(MIN_LEVEL, "34095-00.htm");
		registerQuestItems(ASHEN_CERTIFICATE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "34095-02.htm":
			case "34095-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34095-04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34095-07.html":
			{
				if (qs.isCond(2))
				{
					takeItems(player, -1, ASHEN_CERTIFICATE);
					rewardItems(player, SHADOW_WEAPON_COUPON, 1);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = event;
					break;
				}
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
				htmltext = "34095-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34095-05.html";
				}
				else
				{
					htmltext = "34095-06.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = "34095-08.html";
				}
				else
				{
					qs.setState(State.CREATED);
					htmltext = "34095-01.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		List<Player> members = new ArrayList<>();
		if (player.getParty() != null)
		{
			members = player.getParty().getMembers();
		}
		else
		{
			members.add(player);
		}
		
		for (Player member : members)
		{
			final QuestState qs = getQuestState(member, false);
			if ((qs != null) && qs.isCond(1) && member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE) && giveItemRandomly(member, npc, ASHEN_CERTIFICATE, 1, 8, 1, true))
			{
				qs.setCond(2, true);
			}
		}
	}
}
