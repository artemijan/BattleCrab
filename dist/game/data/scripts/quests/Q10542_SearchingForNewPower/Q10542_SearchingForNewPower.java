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
package quests.Q10542_SearchingForNewPower;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.Q10541_TrainLikeTheRealThing.Q10541_TrainLikeTheRealThing;

/**
 * Searching for New Power (10542)
 * @URL https://l2wiki.com/Searching_for_New_Power
 * @author Gigi
 */
public class Q10542_SearchingForNewPower extends Quest
{
	// NPCs
	private static final int SHANNON = 32974;
	private static final int TOYRON = 33004;
	private static final int THIEF = 23121;
	
	// Items
	private static final int THE_WAR_OF_GODS_AND_GIANTS = 17575;
	private static final int SOULSHOTS = 5789;
	private static final int SPIRITSHOTS = 5790;
	
	// Misc
	public static final int KILL_COUNT_VAR = 0;
	private static final int MAX_LEVEL = 20;
	
	public Q10542_SearchingForNewPower()
	{
		super(10542);
		addStartNpc(SHANNON);
		addTalkId(SHANNON, TOYRON);
		registerQuestItems(THE_WAR_OF_GODS_AND_GIANTS);
		addCondNotRace(Race.ERTHEIA, "noRace.html");
		addCondMaxLevel(MAX_LEVEL, "noLevel.html");
		addCondCompletedQuest(Q10541_TrainLikeTheRealThing.class.getSimpleName(), "noLevel.html");
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
			case "32974-02.htm":
			{
				htmltext = event;
				break;
			}
			case "check":
			{
				qs.startQuest();
				if (player.isInCategory(CategoryType.MAGE_GROUP))
				{
					giveItems(player, SPIRITSHOTS, 100);
					showOnScreenMsg(player, NpcStringId.AUTOMATE_SPIRITSHOT_AS_SHOWN_IN_THE_TUTORIAL, ExShowScreenMessage.TOP_CENTER, 4500);
					htmltext = "32974-04.htm";
				}
				else
				{
					giveItems(player, SOULSHOTS, 100);
					showOnScreenMsg(player, NpcStringId.AUTOMATE_SOULSHOT_AS_SHOWN_IN_THE_TUTORIAL, ExShowScreenMessage.TOP_CENTER, 4500);
					htmltext = "32974-03.htm";
				}
				break;
			}
			case "33004-02.html":
			{
				if (qs.isCond(3))
				{
					showOnScreenMsg(player, NpcStringId.AMONG_THE_4_BOOKSHELVES_FIND_THE_ONE_CONTAINING_A_VOLUME_CALLED_THE_WAR_OF_GODS_AND_GIANTS, ExShowScreenMessage.TOP_CENTER, 4500);
					htmltext = "32974-08.html";
				}
				break;
			}
			case "32974-08.html":
			{
				if (qs.isCond(5))
				{
					giveItems(player, (player.isInCategory(CategoryType.MAGE_GROUP) ? SPIRITSHOTS : SOULSHOTS), 100);
					addExpAndSp(player, 3200, 8);
					qs.exitQuest(false, true);
					htmltext = "32974-08.html";
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
				if (npc.getId() == SHANNON)
				{
					htmltext = "32974-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == SHANNON)
				{
					switch (qs.getCond())
					{
						case 1:
						case 2:
						case 3:
						{
							htmltext = "32974-05.html";
							break;
						}
						case 4:
						{
							htmltext = "32974-06.html";
							break;
						}
						case 5:
						{
							htmltext = "32974-07.html";
							break;
						}
					}
					break;
				}
				else if (npc.getId() == TOYRON)
				{
					if (qs.isCond(2))
					{
						qs.setCond(3, true);
						player.sendPacket(new ExTutorialShowId(15));
						htmltext = "33004-01.html";
						NpcStringId npcStringId = null;
						switch (player.getPlayerClass())
						{
							case FIGHTER:
							case ELVEN_FIGHTER:
							case DARK_FIGHTER:
							{
								npcStringId = NpcStringId.PREPARE_TO_USE_THE_SKILL_POWER_STRIKE_OR_MORTAL_BLOW;
								break;
							}
							case MAGE:
							case ELVEN_MAGE:
							case DARK_MAGE:
							{
								npcStringId = NpcStringId.PREPARE_TO_USE_THE_SKILL_WIND_STRIKE;
								break;
							}
							case ORC_FIGHTER:
							{
								npcStringId = NpcStringId.PREPARE_TO_USE_THE_SKILL_POWER_STRIKE_OR_IRON_PUNCH;
								break;
							}
							case ORC_MAGE:
							{
								npcStringId = NpcStringId.PREPARE_TO_USE_THE_SKILL_CHILL_FLAME;
								break;
							}
							case DWARVEN_FIGHTER:
							{
								npcStringId = NpcStringId.PREPARE_TO_USE_THE_SKILL_SPOIL;
								break;
							}
							case MALE_SOLDIER:
							case FEMALE_SOLDIER:
							{
								npcStringId = NpcStringId.PREPARE_TO_USE_THE_SKILL_FALLEN_ATTACK_OR_FALLEN_ARROW;
								break;
							}
							default:
							{
								break;
							}
						}
						
						if (npcStringId != null)
						{
							showOnScreenMsg(player, npcStringId, ExShowScreenMessage.TOP_CENTER, 4500);
						}
					}
					else if (qs.isCond(5))
					{
						htmltext = "33004-03.html";
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
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final Set<NpcLogListHolder> holder = new HashSet<>();
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(4))
		{
			holder.add(new NpcLogListHolder(THIEF, false, qs.getMemoStateEx(KILL_COUNT_VAR)));
		}
		
		return holder;
	}
}
