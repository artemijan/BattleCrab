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
package quests.Q10874_AgainstTheNewEnemy;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

import quests.Q10873_ExaltedReachingAnotherLevel.Q10873_ExaltedReachingAnotherLevel;

/**
 * Against the New Enemy (10874)
 * @URL https://l2wiki.com/Against_the_New_Enemy
 * @author Mobius
 */
public class Q10874_AgainstTheNewEnemy extends Quest
{
	// NPC
	private static final int CAPTAIN_SPORCHA = 34230;
	private static final int LEONA_BLACKBIRD = 31595;
	private static final int[] MONSTERS =
	{
		// Desert Quarry
		23811, // Cantera Tanya
		23812, // Cantera Deathmoz
		23813, // Cantera Floxis
		23814, // Cantera Belika
		23815, // Cantera Bridget
		
		// Beleth's Magic Circle
		23354, // Decay Hannibal
		23355, // Armor Beast
		23356, // Klein Soldier
		23357, // Disorder Warrior
		23360, // Bizuard
		23361, // Mutated Fly
		
		23367, // Armor Beast
		23368, // Klein Soldier
		23369, // Disorder Warrior
		23372, // Bizuard
		23373, // Mutated Fly
		
		// Giant's Cave Upper Floor
		23727, // Shaqrima Bathus
		23728, // Shaqrima Carcass
		23729, // Shaqrima Kshana
		23733, // Lesser Giant Warrior
		23734, // Lesser Giant Wizard
		23735, // Captive Familiar Spirit
		23736, // Captive Hell Demon
		23737, // Captive Succubus
		23738, // Captive Phantom
		23742, // Naia Bathus, Demons Foreman
		23743, // Naia Karkus, Demons Foreman
		23744, // Naia Kshana, Demons Foreman
		23746, // Recovering Lesser Giant Warrior
		23747, // Recovering Lesser Giant Wizard
		23749, // Root of the Lesser Giant
		23754, // Essence of the Lesser Giant
	};
	
	// Items
	private static final int PROOF_OF_QUALIFICATION = 47839;
	private static final ItemHolder LIONEL_HUNTERS_LIST_PART_4 = new ItemHolder(47829, 1);
	
	// Rewards
	private static final int LEONA_CERTIFICATE = 47830;
	private static final int FP = 4500; // Faction points
	
	// Misc
	private static final int MIN_LEVEL = 103;
	private static final int PROOF_OF_QUALIFICATION_NEEDED = 10000;
	
	public Q10874_AgainstTheNewEnemy()
	{
		super(10874);
		addStartNpc(CAPTAIN_SPORCHA);
		addTalkId(CAPTAIN_SPORCHA, LEONA_BLACKBIRD);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "34230-00.html");
		addCondStartedQuest(Q10873_ExaltedReachingAnotherLevel.class.getSimpleName(), "34230-00.html");
		registerQuestItems(PROOF_OF_QUALIFICATION);
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
			case "31595-02.htm":
			case "31595-05.htm":
			case "34230-02.htm":
			case "34230-03.htm":
			case "34230-04.htm":
			{
				htmltext = event;
				break;
			}
			case "34230-05.html":
			{
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_4))
				{
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "31595-03.htm":
			{
				if (qs.isCond(2) && (getQuestItemsCount(player, PROOF_OF_QUALIFICATION) >= PROOF_OF_QUALIFICATION_NEEDED))
				{
					addFactionPoints(player, Faction.GIANT_TRACKERS, FP); // add FP points to GIANT_TRACKERS Faction
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					qs.setCond(3, true);
					htmltext = event;
				}
				break;
			}
			case "31595-04.htm":
			{
				if (qs.isCond(2) && (getQuestItemsCount(player, PROOF_OF_QUALIFICATION) >= PROOF_OF_QUALIFICATION_NEEDED))
				{
					addFactionPoints(player, Faction.BLACKBIRD_CLAN, FP); // add FP points to BLACKBIRD_CLAN Faction
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					qs.setCond(3, true);
					htmltext = event;
				}
				break;
			}
			case "31595-06.html":
			{
				if (qs.isCond(3) && (getQuestItemsCount(player, PROOF_OF_QUALIFICATION) >= PROOF_OF_QUALIFICATION_NEEDED))
				{
					takeItems(player, PROOF_OF_QUALIFICATION, PROOF_OF_QUALIFICATION_NEEDED);
					giveItems(player, LEONA_CERTIFICATE, 1);
					qs.exitQuest(false, true);
					
					final Quest mainQ = ScriptManager.getInstance().getScript(Q10873_ExaltedReachingAnotherLevel.class.getSimpleName());
					if (mainQ != null)
					{
						mainQ.notifyEvent("SUBQUEST_FINISHED_NOTIFY", npc, player);
					}
					
					htmltext = event;
				}
				else
				{
					htmltext = getNoQuestLevelRewardMsg(player);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_4))
				{
					htmltext = "34230-01.htm";
				}
				else
				{
					htmltext = "34230-00.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case CAPTAIN_SPORCHA:
					{
						if (qs.isCond(1))
						{
							if (getQuestItemsCount(player, PROOF_OF_QUALIFICATION) >= PROOF_OF_QUALIFICATION_NEEDED)
							{
								htmltext = "34230-07.html";
							}
							else
							{
								htmltext = "34230-06.html";
							}
						}
						break;
					}
					case LEONA_BLACKBIRD:
					{
						if (qs.isCond(2))
						{
							htmltext = "31595-01.htm";
						}
						else if (qs.isCond(3))
						{
							htmltext = "31595-05.htm";
						}
						else
						{
							htmltext = "31595-00.html";
						}
					}
						break;
				}
			}
				break;
			
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			if ((getQuestItemsCount(player, PROOF_OF_QUALIFICATION) < PROOF_OF_QUALIFICATION_NEEDED) && (getRandom(100) < 90))
			{
				giveItems(player, PROOF_OF_QUALIFICATION, 1);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			
			if ((getQuestItemsCount(player, PROOF_OF_QUALIFICATION) >= PROOF_OF_QUALIFICATION_NEEDED) && (player.getLevel() >= MIN_LEVEL))
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
	}
}
