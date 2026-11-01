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
package quests.Q10880_TheLastOneStanding;

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
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10879_ExaltedGuideToPower.Q10879_ExaltedGuideToPower;

/**
 * The Last One Standing (10880)
 * @URL https://l2wiki.com/The_Last_One_Standing
 * @author Dmitri
 */
public class Q10880_TheLastOneStanding extends Quest
{
	// NPCs
	private static final int CYPHONA = 34055;
	private static final int FERIN = 34054;
	
	// Items
	private static final int FERIN_CERTIFICATE = 47835;
	private static final int PROOF_OF_STRENGTH = 47843;
	private static final ItemHolder LIONEL_HUNTERS_LIST_PART_5 = new ItemHolder(47834, 1);
	
	// Monsters
	private static final int[] MONSTERS =
	{
		// The Enchanted Valley
		23566, // Nymph Rose
		23567, // Nymph Rose
		23568, // Nymph Lily
		23569, // Nymph Lily
		23570, // Nymph Tulip
		23571, // Nymph Tulip
		23572, // Nymph Cosmos
		23573, // Nymph Cosmos
		23578, // Nymph Guardian
		
		// Garden of Spirits
		23541, // Kerberos Lager
		23550, // Kerberos Lager (night)
		23542, // Kerberos Fort
		23551, // Kerberos Fort (night)
		23543, // Kerberos Nero
		23552, // Kerberos Nero (night)
		23544, // Fury Sylph Barrena
		23553, // Fury Sylph Barrena (night)
		23546, // Fury Sylph Temptress
		23555, // Fury Sylph Temptress (night)
		23547, // Fury Sylph Purka
		23556, // Fury Sylph Purka (night)
		23545, // Fury Kerberos Leger
		23557, // Fury Kerberos Leger (night)
		23549, // Fury Kerberos Nero
		23558, // Fury Kerberos Nero (night)
		
		// Atelia Fortress
		23505, // Fortress Raider 101
		23506, // Fortress Guardian Captain 101
		23537, // Atelia Elite Captain Atelia Infuser 102
		23538, // Atelia High Priest Atelia Infuser 103
		23536, // Atelia High Priest Kelbim's 102
		23535, // Atelia Archon Kelbim's 102
		23532, // Atelia Elite Captain Kelbim's 101
		23530, // Fortress Guardian Captain Kelbim's 101
		23507, // Atelia Passionate Soldier 101
		23508, // Atelia Elite Captain 101
		23509, // Fortress Dark Wizard 102
		23510, // Atelia Flame Master 102
		23511, // Fortress Archon 102
		23512, // Atelia High Priest 102
		
		// Shadow of the Mother Tree
		24117, // Crystal Reep
		24118, // Crystal Reep
		24119, // Crystal Needle
		24120, // Crystal Needle
		24121, // Treant Blossom
		24122, // Treant Blossom
		24123, // Flush Teasle
		24124, // Flush Teasle
		24125, // Creeper Rampike
		24126, // Creeper Rampike
		24139, // Reep Child
		24140, // Needle Child
		24141, // Blossom Child
		24142 // Teasle Child
	};
	
	// Misc
	private static final int MIN_LEVEL = 104;
	private static final int FP = 4500; // Faction Points
	private static final int PROOF_OF_STRENGTH_NEEDED = 10000;
	
	public Q10880_TheLastOneStanding()
	{
		super(10880);
		addStartNpc(CYPHONA);
		addTalkId(CYPHONA, FERIN);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "34055-00.htm");
		addCondStartedQuest(Q10879_ExaltedGuideToPower.class.getSimpleName(), "34055-00.htm");
		registerQuestItems(PROOF_OF_STRENGTH);
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
			case "34055-02.htm":
			case "34055-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34055-04.htm":
			{
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_5))
				{
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "34054-07.html":
			{
				if (qs.isCond(2))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						takeItems(player, PROOF_OF_STRENGTH, -1);
						giveItems(player, FERIN_CERTIFICATE, 1);
						addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, FP);
						qs.exitQuest(false, true);
						
						final Quest mainQ = ScriptManager.getInstance().getScript(Q10879_ExaltedGuideToPower.class.getSimpleName());
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
				}
				break;
			}
			case "34054-07a.html":
			{
				if (qs.isCond(2))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						takeItems(player, PROOF_OF_STRENGTH, -1);
						giveItems(player, FERIN_CERTIFICATE, 1);
						addFactionPoints(player, Faction.UNWORLDLY_VISITORS, FP);
						qs.exitQuest(false, true);
						
						final Quest mainQ = ScriptManager.getInstance().getScript(Q10879_ExaltedGuideToPower.class.getSimpleName());
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
				}
				break;
			}
			case "34054-07b.html":
			{
				if (qs.isCond(2) && (getQuestItemsCount(player, PROOF_OF_STRENGTH) >= PROOF_OF_STRENGTH_NEEDED))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						takeItems(player, PROOF_OF_STRENGTH, PROOF_OF_STRENGTH_NEEDED);
						giveItems(player, FERIN_CERTIFICATE, 1);
						addFactionPoints(player, Faction.KINGDOM_ROYAL_GUARDS, FP);
						qs.exitQuest(false, true);
						
						final Quest mainQ = ScriptManager.getInstance().getScript(Q10879_ExaltedGuideToPower.class.getSimpleName());
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
				if ((npc.getId() == CYPHONA) && (hasItem(player, LIONEL_HUNTERS_LIST_PART_5)))
				{
					htmltext = "34055-01.htm";
				}
				else
				{
					htmltext = "noItem.html";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case CYPHONA:
					{
						if (qs.isCond(1))
						{
							htmltext = "34055-05.html";
						}
						break;
					}
					case FERIN:
					{
						if (qs.isCond(2) && (getQuestItemsCount(player, PROOF_OF_STRENGTH) >= PROOF_OF_STRENGTH_NEEDED))
						{
							htmltext = "34054-06.html";
						}
						break;
					}
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
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		executeForEachPlayer(player, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE) && ArrayUtil.contains(MONSTERS, npc.getId()))
		{
			giveItems(player, PROOF_OF_STRENGTH, 1);
			playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			
			if (getQuestItemsCount(player, PROOF_OF_STRENGTH) >= PROOF_OF_STRENGTH_NEEDED)
			{
				qs.setCond(2, true);
			}
		}
	}
}
