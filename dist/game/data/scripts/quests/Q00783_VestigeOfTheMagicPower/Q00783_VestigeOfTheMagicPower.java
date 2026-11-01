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
package quests.Q00783_VestigeOfTheMagicPower;

import java.util.List;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10455_ElikiasLetter.Q10455_ElikiasLetter;

/**
 * Vestige of the Magic Power (783)
 * @URL https://l2wiki.com/Vestige_of_the_Magic_Power
 * @author Gigi, Iris
 */
public class Q00783_VestigeOfTheMagicPower extends Quest
{
	// NPCs
	private static final int LEONA_BLACKBIRD = 31595;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23384, // Smaug
		23385, // Lunatikan
		23386, // Jabberwok
		23387, // Kanzaroth
		23388, // Kandiloth
		23395, // Garion
		23396, // Garion Neti
		23397, // Desert Wendigo
		23398, // Koraza
		23399 // Bend Beetle
	};
	
	// Misc
	private static final int MIN_LEVEL = 99;
	private static final int HIGH_GRADE_FRAGMENT_OF_CHAOS = 46557;
	private static final int BASIC_SUPPLY_BOX = 47356;
	private static final int INTERMEDIATE_SUPPLY_BOX = 47357;
	private static final int ADVANCED_SUPPLY_BOX = 47358;
	
	public Q00783_VestigeOfTheMagicPower()
	{
		super(783);
		addStartNpc(LEONA_BLACKBIRD);
		addTalkId(LEONA_BLACKBIRD);
		addKillId(MONSTERS);
		registerQuestItems(HIGH_GRADE_FRAGMENT_OF_CHAOS);
		addCondMinLevel(MIN_LEVEL, "31595-00.htm");
		addCondCompletedQuest(Q10455_ElikiasLetter.class.getSimpleName(), "31595-00.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "31595-02.htm":
			case "31595-03.htm":
			case "31595-05.htm":
			case "31595-06.htm":
			case "31595-07.htm":
			{
				htmltext = event;
				break;
			}
			case "31595-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "31595-05a.htm":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "31595-06a.htm":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "31595-07a.htm":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "31595-05b.htm":
			{
				qs.exitQuest(QuestType.DAILY, true);
				addFactionPoints(player, Faction.BLACKBIRD_CLAN, 100);
				giveItemRandomly(player, BASIC_SUPPLY_BOX, 1, 1, 0.9, false);
				giveItemRandomly(player, INTERMEDIATE_SUPPLY_BOX, 1, 1, 0.4, false);
				giveItemRandomly(player, ADVANCED_SUPPLY_BOX, 1, 1, 0.2, false);
				addExpAndSp(player, 12113489880L, 12113460);
				htmltext = event;
				break;
			}
			case "31595-06b.htm":
			{
				qs.exitQuest(QuestType.DAILY, true);
				addFactionPoints(player, Faction.BLACKBIRD_CLAN, 200);
				giveItemRandomly(player, BASIC_SUPPLY_BOX, 1, 1, 0.4, false);
				giveItemRandomly(player, INTERMEDIATE_SUPPLY_BOX, 1, 1, 0.9, false);
				giveItemRandomly(player, ADVANCED_SUPPLY_BOX, 1, 1, 0.2, false);
				addExpAndSp(player, 24226979760L, 24226920);
				htmltext = event;
				break;
			}
			case "31595-07b.htm":
			{
				qs.exitQuest(QuestType.DAILY, true);
				addFactionPoints(player, Faction.BLACKBIRD_CLAN, 300);
				giveItemRandomly(player, BASIC_SUPPLY_BOX, 1, 1, 0.2, false);
				giveItemRandomly(player, INTERMEDIATE_SUPPLY_BOX, 1, 1, 0.5, false);
				giveItemRandomly(player, ADVANCED_SUPPLY_BOX, 1, 1, 0.9, false);
				giveItems(player, ADVANCED_SUPPLY_BOX, 1);
				addExpAndSp(player, 36340469640L, 36340380);
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
		if (npc.getId() == LEONA_BLACKBIRD)
		{
			switch (qs.getState())
			{
				case State.CREATED:
				{
					htmltext = "31595-01.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "31595-04.htm";
					}
					else if (qs.isCond(5))
					{
						htmltext = "31595-05b.htm";
						onEvent(htmltext, npc, player);
					}
					else if (qs.isCond(6))
					{
						htmltext = "31595-06b.htm";
						onEvent(htmltext, npc, player);
					}
					else if (qs.isCond(7))
					{
						htmltext = "31595-07b.htm";
						onEvent(htmltext, npc, player);
					}
					else if (qs.isCond(2) || qs.isCond(3) || qs.isCond(4))
					{
						htmltext = "31595-08.htm";
					}
					break;
				}
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "31595-00a.htm";
						break;
					}
					
					qs.setState(State.CREATED);
					htmltext = "31595-01.htm";
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (killer.isInParty())
		{
			final Party party = killer.getParty();
			final List<Player> partyMember = party.getMembers();
			for (Player singleMember : partyMember)
			{
				final QuestState qsPartyMember = getQuestState(singleMember, false);
				final double distance = npc.calculateDistance3D(singleMember);
				if ((qsPartyMember != null) && (distance <= 1000))
				{
					if (qsPartyMember.isCond(2) && giveItemRandomly(singleMember, npc, HIGH_GRADE_FRAGMENT_OF_CHAOS, 1, 300, 1, true))
					{
						qsPartyMember.setCond(5, true);
					}
					
					if (qsPartyMember.isCond(3) && giveItemRandomly(singleMember, npc, HIGH_GRADE_FRAGMENT_OF_CHAOS, 1, 300, 1, true))
					{
						qsPartyMember.setCond(6, true);
					}
					
					if (qsPartyMember.isCond(4) && giveItemRandomly(singleMember, npc, HIGH_GRADE_FRAGMENT_OF_CHAOS, 1, 300, 1, true))
					{
						qsPartyMember.setCond(7, true);
					}
				}
			}
		}
		else
		{
			final QuestState qs = getRandomPartyMemberState(killer, -1, 3, npc);
			if (qs != null)
			{
				if (qs.isCond(2) && giveItemRandomly(killer, npc, HIGH_GRADE_FRAGMENT_OF_CHAOS, 1, 300, 1, true))
				{
					qs.setCond(5, true);
				}
				
				if (qs.isCond(3) && giveItemRandomly(killer, npc, HIGH_GRADE_FRAGMENT_OF_CHAOS, 1, 300, 1, true))
				{
					qs.setCond(6, true);
				}
				
				if (qs.isCond(4) && giveItemRandomly(killer, npc, HIGH_GRADE_FRAGMENT_OF_CHAOS, 1, 300, 1, true))
				{
					qs.setCond(7, true);
				}
			}
		}
	}
}
