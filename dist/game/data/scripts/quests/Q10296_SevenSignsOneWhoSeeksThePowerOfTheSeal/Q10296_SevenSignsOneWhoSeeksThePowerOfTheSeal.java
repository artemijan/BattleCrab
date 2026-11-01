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
package quests.Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.enums.Movie;

import quests.Q10295_SevenSignsSolinasTomb.Q10295_SevenSignsSolinasTomb;

/**
 * Seven Signs, One Who Seeks the Power of the Seal (10296)
 * @URL https://l2wiki.com/Seven_Signs,_One_Who_Seeks_the_Power_of_the_Seal
 * @author Mobius
 */
public class Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal extends Quest
{
	// NPCs
	private static final int ERIS_EVIL_THOUGHTS = 32792;
	private static final int ELCADIA_INSTANCE = 32787;
	private static final int ETIS_VAN_ETINA = 18949;
	private static final int ELCADIA = 32784;
	private static final int HARDIN = 30832;
	private static final int WOOD = 32593;
	private static final int FRANZ = 32597;
	
	// Location
	private static final Location UNKNOWN_LOC = new Location(76707, -241022, -10832);
	
	// Reward
	private static final int CERTIFICATE_OF_DAWN = 17265;
	
	// Misc
	private static final int MIN_LEVEL = 81;
	
	public Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal()
	{
		super(10296);
		addStartNpc(ERIS_EVIL_THOUGHTS);
		addTalkId(ERIS_EVIL_THOUGHTS, ELCADIA_INSTANCE, ELCADIA, HARDIN, WOOD, FRANZ);
		addKillId(ETIS_VAN_ETINA);
		addCondMinLevel(MIN_LEVEL, "");
		addCondCompletedQuest(Q10295_SevenSignsSolinasTomb.class.getSimpleName(), "");
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
			case "32792-02.htm":
			case "32784-02.html":
			case "30832-02.html":
			case "32597-02.html":
			{
				htmltext = event;
				break;
			}
			case "32792-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "start_video_1":
			{
				playMovie(player, Movie.SSQ2_BOSS_OPENING);
				startQuestTimer("teleport_to_unknown", 60000, null, player);
				return null;
			}
			case "teleport_to_unknown":
			{
				final Npc etis = player.getInstanceWorld().getNpc(ETIS_VAN_ETINA);
				if (etis != null)
				{
					etis.deleteMe();
				}
				
				player.teleToLocation(UNKNOWN_LOC);
				final Npc elcadia = player.getInstanceWorld().getNpc(ELCADIA_INSTANCE);
				elcadia.teleToLocation(player, true);
				qs.setCond(2, true);
				startQuestTimer("spawn_etis", 10000, null, player);
				return null;
			}
			case "spawn_etis":
			{
				final Npc etis = player.getInstanceWorld().getNpc(ETIS_VAN_ETINA);
				if (etis == null)
				{
					addSpawn(ETIS_VAN_ETINA, UNKNOWN_LOC, false, 0, false, player.getInstanceId());
				}
				
				return null;
			}
			case "respawn_elcadia":
			{
				addSpawn(ELCADIA_INSTANCE, UNKNOWN_LOC, false, 0, false, player.getInstanceId());
				return null;
			}
			case "exit_instance":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
				}
				
				final Instance world = player.getInstanceWorld();
				world.ejectPlayer(player);
				world.destroy();
				return null;
			}
			case "32784-03.html":
			{
				if (qs.isCond(3))
				{
					qs.setCond(4, true);
					htmltext = event;
				}
				break;
			}
			case "30832-03.html":
			{
				if (qs.isCond(4))
				{
					qs.setCond(5, true);
					htmltext = event;
				}
				break;
			}
			case "32597-03.html":
			{
				if (qs.isCond(5))
				{
					qs.unset("erisKilled");
					rewardItems(player, CERTIFICATE_OF_DAWN, 1);
					addExpAndSp(player, 70000000, 16800);
					qs.exitQuest(false, true);
					htmltext = event;
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
				if (npc.getId() == ERIS_EVIL_THOUGHTS)
				{
					htmltext = "32792-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case ERIS_EVIL_THOUGHTS:
					{
						if (qs.isCond(1) || qs.isCond(2))
						{
							htmltext = "32792-04.html";
						}
						break;
					}
					case ELCADIA_INSTANCE:
					{
						if (qs.getInt("erisKilled") == 1)
						{
							htmltext = "32787-01.html";
						}
						break;
					}
					case ELCADIA:
					{
						if (qs.isCond(3))
						{
							htmltext = "32784-01.html";
						}
						else if (qs.isCond(4))
						{
							htmltext = "32784-04.html";
						}
						break;
					}
					case HARDIN:
					{
						if (qs.isCond(4))
						{
							htmltext = "30832-01.html";
						}
						else if (qs.isCond(5))
						{
							htmltext = "30832-04.html";
						}
						break;
					}
					case WOOD:
					{
						if (qs.isCond(5))
						{
							htmltext = "32593-01.html";
						}
						break;
					}
					case FRANZ:
					{
						if (qs.isCond(5))
						{
							htmltext = "32597-01.html";
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
		final QuestState qs = getQuestState(player, false);
		qs.set("erisKilled", 1);
		npc.deleteMe();
		final Npc elcadia = player.getInstanceWorld().getNpc(ELCADIA_INSTANCE);
		elcadia.deleteMe();
		startQuestTimer("respawn_elcadia", 60000, null, player);
		playMovie(player, Movie.SSQ2_BOSS_CLOSING);
	}
}
