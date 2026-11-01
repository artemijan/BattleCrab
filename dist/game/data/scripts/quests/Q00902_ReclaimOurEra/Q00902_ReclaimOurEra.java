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
package quests.Q00902_ReclaimOurEra;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * Reclaim Our Era (902)
 * @author Stayway
 */
public class Q00902_ReclaimOurEra extends Quest
{
	// Npc
	private static final int MATHIAS = 31340;
	
	// Misc
	private static final int MIN_LEVEL = 80;
	
	// Items
	private static final int SHATTERED_BONES = 21997;
	private static final int CANNIBALISTIC_STAKATO_LDR_CLAW = 21998;
	private static final int ANAIS_SCROLL = 21999;
	private static final int PROOF_OF_CHALLENGE = 21750;
	
	// Monsters
	private static final Map<Integer, Integer> MONSTER_DROPS = new HashMap<>();
	static
	{
		MONSTER_DROPS.put(25309, SHATTERED_BONES); // Varka's Hero Shadith
		MONSTER_DROPS.put(25299, SHATTERED_BONES); // Ketra's Hero Hekaton
		MONSTER_DROPS.put(25667, CANNIBALISTIC_STAKATO_LDR_CLAW); // Cannibalistic Stakato Chief
		MONSTER_DROPS.put(25668, CANNIBALISTIC_STAKATO_LDR_CLAW); // Cannibalistic Stakato Chief
		MONSTER_DROPS.put(25669, CANNIBALISTIC_STAKATO_LDR_CLAW); // Cannibalistic Stakato Chief
		MONSTER_DROPS.put(25670, CANNIBALISTIC_STAKATO_LDR_CLAW); // Cannibalistic Stakato Chief
		MONSTER_DROPS.put(25701, ANAIS_SCROLL); // Anais - Master of Splendor
	}
	
	public Q00902_ReclaimOurEra()
	{
		super(902);
		addStartNpc(MATHIAS);
		addTalkId(MATHIAS);
		addKillId(MONSTER_DROPS.keySet());
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
		registerQuestItems(SHATTERED_BONES, CANNIBALISTIC_STAKATO_LDR_CLAW, ANAIS_SCROLL);
	}
	
	private void giveItem(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isStarted() && !qs.isCond(5) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			giveItems(player, MONSTER_DROPS.get(npc.getId()), 1);
			qs.setCond(5, true);
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		String htmltext = null;
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "31340-04.htm":
			{
				htmltext = event;
				break;
			}
			case "31340-05.html":
			{
				if (qs.isCreated())
				{
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "31340-06.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2, true);
					htmltext = event;
				}
				break;
			}
			case "31340-07.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(3, true);
					htmltext = event;
				}
				break;
			}
			case "31340-08.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(4, true);
					htmltext = event;
				}
				break;
			}
			case "31340-10.html":
			{
				if (qs.isCond(1))
				{
					htmltext = event;
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (killer.isInParty())
		{
			for (Player member : killer.getParty().getMembers())
			{
				giveItem(npc, member);
			}
		}
		else
		{
			giveItem(npc, killer);
		}
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = "31340-02.htm";
					break;
				}
				
				qs.setState(State.CREATED);
				// fallthrough
			}
			case State.CREATED:
			{
				htmltext = (player.getLevel() >= MIN_LEVEL) ? "31340-01.htm" : "31340-03.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "31340-09.html";
						break;
					}
					case 2:
					{
						htmltext = "31340-11.html";
						break;
					}
					case 3:
					{
						htmltext = "31340-12.html";
						break;
					}
					case 4:
					{
						htmltext = "31340-13.html";
						break;
					}
					case 5:
					{
						if (hasQuestItems(player, SHATTERED_BONES))
						{
							giveItems(player, PROOF_OF_CHALLENGE, 1);
							giveAdena(player, 134038, true);
						}
						else if (hasQuestItems(player, CANNIBALISTIC_STAKATO_LDR_CLAW))
						{
							giveItems(player, PROOF_OF_CHALLENGE, 3);
							giveAdena(player, 210119, true);
						}
						else if (hasQuestItems(player, ANAIS_SCROLL))
						{
							giveItems(player, PROOF_OF_CHALLENGE, 3);
							giveAdena(player, 348155, true);
						}
						
						qs.exitQuest(QuestType.DAILY, true);
						htmltext = "31340-14.html";
						break;
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
}
