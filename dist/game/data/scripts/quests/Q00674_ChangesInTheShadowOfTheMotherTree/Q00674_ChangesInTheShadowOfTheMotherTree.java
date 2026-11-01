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
package quests.Q00674_ChangesInTheShadowOfTheMotherTree;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

/**
 * Changes in the Shadow of the Mother Tree (674)
 * @URL https://l2wiki.com/Changes_in_the_Shadow_of_the_Mother_Tree
 * @author Dmitri
 */
public class Q00674_ChangesInTheShadowOfTheMotherTree extends Quest
{
	// NPCs
	private static final int CERIEL = 34415;
	private static final int NERUPA = 34412;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		24118, // Crystal Reep
		24120, // Crystal Needle
		24122, // Treant Blossom
		24124, // Flush Teasle
		24126, // Creeper Rampike
		24139, // Reep Child
		24140, // Needle Child
		24141, // Blossom Child
		24142 // Teasle Child
	};
	
	// Items
	private static final int BASIC_SUPPLY_BOX = 48397; // Mother Tree Guardians Basic Treasure Chest: Shadow of the Mother Tree
	private static final int INTERMEDIATE_SUPPLY_BOX = 48398; // Mother Tree Guardians Intermediate Treasure Chest: Shadow of the Mother Tree
	private static final int ADVANCED_SUPPLY_BOX = 48399; // Mother Tree Guardians Advanced Treasure Chest: Shadow of the Mother Tree
	
	// Misc
	private static final int KILLING_NPCSTRING_ID = NpcStringId.ELIMINATE_THE_GIANT.getId(); // NpcStringId.1019709
	private static final boolean PARTY_QUEST = false;
	private static final int MIN_LEVEL = 103;
	
	public Q00674_ChangesInTheShadowOfTheMotherTree()
	{
		super(674);
		addStartNpc(CERIEL);
		addTalkId(CERIEL, NERUPA);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "34415-00.htm");
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
			case "34415-02.htm":
			case "34415-03.htm":
			case "34415-04.htm":
			case "34415-04a.htm":
			case "34415-04b.htm":
			case "34415-06.html":
			case "34415-06a.html":
			case "34415-06b.html":
			{
				htmltext = event;
				break;
			}
			case "select_mission":
			{
				qs.startQuest();
				if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 6) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 7))
				{
					htmltext = "34415-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 7)
				{
					htmltext = "34415-04b.htm";
					break;
				}
				
				htmltext = "34415-04.htm";
				break;
			}
			case "return":
			{
				if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 6) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 7))
				{
					htmltext = "34415-04a.htm";
					break;
				}
				else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 7)
				{
					htmltext = "34415-04b.htm";
					break;
				}
				
				htmltext = "34415-04.htm";
				break;
			}
			case "34415-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34415-07a.html":
			{
				qs.setCond(3, true);
				htmltext = event;
				break;
			}
			case "34415-07b.html":
			{
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34412-10.html":
			{
				final int chance = getRandom(100);
				switch (qs.getCond())
				{
					case 5:
					{
						if (player.getLevel() >= MIN_LEVEL)
						{
							if (chance < 2)
							{
								giveItems(player, ADVANCED_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, INTERMEDIATE_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, BASIC_SUPPLY_BOX, 1);
							}
							
							addExpAndSp(player, 27_191_624_760L, 27_191_610);
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, 100);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
					case 6:
					{
						if (player.getLevel() >= MIN_LEVEL)
						{
							if (chance < 2)
							{
								giveItems(player, ADVANCED_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, BASIC_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, INTERMEDIATE_SUPPLY_BOX, 1);
							}
							
							addExpAndSp(player, 54_383_249_520L, 54_383_220);
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, 200);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
					case 7:
					{
						if (player.getLevel() >= MIN_LEVEL)
						{
							if (chance < 2)
							{
								giveItems(player, BASIC_SUPPLY_BOX, 1);
							}
							else if (chance < 20)
							{
								giveItems(player, INTERMEDIATE_SUPPLY_BOX, 1);
							}
							else if (chance < 100)
							{
								giveItems(player, ADVANCED_SUPPLY_BOX, 1);
							}
							
							addExpAndSp(player, 81_574_874_280L, 81_574_830);
							addFactionPoints(player, Faction.MOTHER_TREE_GUARDIANS, 300);
							qs.exitQuest(QuestType.DAILY, true);
							htmltext = event;
						}
						else
						{
							htmltext = getNoQuestLevelRewardMsg(player);
						}
						break;
					}
				}
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
				if (npc.getId() == CERIEL)
				{
					htmltext = "34415-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case CERIEL:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								if ((player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 6) && (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) < 7))
								{
									htmltext = "34415-04a.htm";
									break;
								}
								else if (player.getFactionLevel(Faction.MOTHER_TREE_GUARDIANS) >= 7)
								{
									htmltext = "34415-04b.htm";
									break;
								}
								
								htmltext = "34415-04.htm";
								break;
							}
							case 2:
							{
								htmltext = "34415-08.html";
								break;
							}
							case 3:
							{
								htmltext = "34415-08a.html";
								break;
							}
							case 4:
							{
								htmltext = "34415-08b.html";
								break;
							}
						}
						break;
					}
					case NERUPA:
					{
						switch (qs.getCond())
						{
							case 5:
							case 6:
							case 7:
							{
								htmltext = "34412-09.html";
								break;
							}
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				else
				{
					qs.setState(State.CREATED);
					htmltext = "34415-01.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = PARTY_QUEST ? getRandomPartyMemberState(killer, -1, 3, npc) : getQuestState(killer, false);
		if ((qs != null) && (qs.getCond() > 1))
		{
			switch (qs.getCond())
			{
				case 2:
				{
					final int killedGhosts = qs.getInt("AncientGhosts") + 1;
					qs.set("AncientGhosts", killedGhosts);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					if (killedGhosts >= 200)
					{
						qs.setCond(5, true);
					}
					break;
				}
				case 3:
				{
					final int killedGhosts = qs.getInt("AncientGhosts") + 1;
					qs.set("AncientGhosts", killedGhosts);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					if (killedGhosts >= 400)
					{
						qs.setCond(6, true);
					}
					break;
				}
				case 4:
				{
					final int killedGhosts = qs.getInt("AncientGhosts") + 1;
					qs.set("AncientGhosts", killedGhosts);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					if (killedGhosts >= 600)
					{
						qs.setCond(7, true);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.getCond() > 1))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(KILLING_NPCSTRING_ID, true, qs.getInt("AncientGhosts")));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
