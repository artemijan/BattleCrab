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
package quests.Q00254_LegendaryTales;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Legendary Tales (254)
 * @author nonom
 */
public class Q00254_LegendaryTales extends Quest
{
	// NPC
	private static final int GILMORE = 30754;
	
	// Monsters
	private enum Bosses
	{
		EMERALD_HORN(25718),
		DUST_RIDER(25719),
		BLEEDING_FLY(25720),
		BLACK_DAGGER(25721),
		SHADOW_SUMMONER(25722),
		SPIKE_SLASHER(25723),
		MUSCLE_BOMBER(25724);
		
		private final int _bossId;
		private final int _mask;
		
		private Bosses(int bossId)
		{
			_bossId = bossId;
			_mask = 1 << ordinal();
		}
		
		public int getId()
		{
			return _bossId;
		}
		
		public int getMask()
		{
			return _mask;
		}
		
		public static Bosses valueOf(int npcId)
		{
			for (Bosses val : values())
			{
				if (val.getId() == npcId)
				{
					return val;
				}
			}
			return null;
		}
	}
	
	// @formatter:off
	private static final int[] MONSTERS =
	{
		Bosses.EMERALD_HORN.getId(), Bosses.DUST_RIDER.getId(), Bosses.BLEEDING_FLY.getId(), 
		Bosses.BLACK_DAGGER.getId(), Bosses.SHADOW_SUMMONER.getId(), Bosses.SPIKE_SLASHER.getId(), 
		Bosses.MUSCLE_BOMBER.getId()
	};
	// @formatter:on
	
	// Items
	private static final int LARGE_DRAGON_SKULL = 17249;
	
	// Misc
	private static final int MIN_LEVEL = 80;
	
	public Q00254_LegendaryTales()
	{
		super(254);
		addStartNpc(GILMORE);
		addTalkId(GILMORE);
		addKillId(MONSTERS);
		registerQuestItems(LARGE_DRAGON_SKULL);
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
				htmltext = (player.getLevel() < MIN_LEVEL) ? "30754-00.htm" : "30754-01.htm";
				break;
			}
			case State.STARTED:
			{
				final long count = getQuestItemsCount(player, LARGE_DRAGON_SKULL);
				if (qs.isCond(1))
				{
					htmltext = ((count > 0) ? "30754-14.htm" : "30754-06.html");
				}
				else if (qs.isCond(2))
				{
					htmltext = ((count < 7) ? "30754-12.htm" : "30754-07.html");
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "30754-29.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "30754-05.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30754-02.html":
			case "30754-03.html":
			case "30754-04.htm":
			case "30754-08.html":
			case "30754-15.html":
			case "30754-20.html":
			case "30754-21.html":
			{
				htmltext = event;
				break;
			}
			case "25718": // Emerald Horn
			{
				htmltext = (checkMask(qs, Bosses.EMERALD_HORN) ? "30754-22.html" : "30754-16.html");
				break;
			}
			case "25719": // Dust Rider
			{
				htmltext = (checkMask(qs, Bosses.DUST_RIDER) ? "30754-23.html" : "30754-17.html");
				break;
			}
			case "25720": // Bleeding Fly
			{
				htmltext = (checkMask(qs, Bosses.BLEEDING_FLY) ? "30754-24.html" : "30754-18.html");
				break;
			}
			case "25721": // Black Dagger Wing
			{
				htmltext = (checkMask(qs, Bosses.BLACK_DAGGER) ? "30754-25.html" : "30754-19.html");
				break;
			}
			case "25722": // Shadow Summoner
			{
				htmltext = (checkMask(qs, Bosses.SHADOW_SUMMONER) ? "30754-26.html" : "30754-16.html");
				break;
			}
			case "25723": // Spike Slasher
			{
				htmltext = (checkMask(qs, Bosses.SPIKE_SLASHER) ? "30754-27.html" : "30754-17.html");
				break;
			}
			case "25724": // Muscle Bomber
			{
				htmltext = (checkMask(qs, Bosses.MUSCLE_BOMBER) ? "30754-28.html" : "30754-18.html");
				break;
			}
			case "13467": // Vesper Thrower
			case "13466": // Vesper Singer
			case "13465": // Vesper Caster
			case "13464": // Vesper Retributer
			case "13463": // Vesper Avenger
			case "13457": // Vesper Cutter
			case "13458": // Vesper Slasher
			case "13459": // Vesper Buster
			case "13460": // Vesper Sharper
			case "13461": // Vesper Fighter
			case "13462": // Vesper Stormer
			{
				if (qs.isCond(2) && (getQuestItemsCount(player, LARGE_DRAGON_SKULL) >= 7))
				{
					htmltext = "30754-09.html";
					rewardItems(player, Integer.parseInt(event), 1);
					qs.exitQuest(false, true);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isPet)
	{
		if (player.isInParty())
		{
			for (Player partyMember : player.getParty().getMembers())
			{
				actionForEachPlayer(partyMember, npc, false);
			}
		}
		else
		{
			actionForEachPlayer(player, npc, false);
		}
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = player.getQuestState(Q00254_LegendaryTales.class.getSimpleName());
		if ((qs != null) && qs.isCond(1))
		{
			final int raids = qs.getInt("raids");
			final Bosses boss = Bosses.valueOf(npc.getId());
			if (!checkMask(qs, boss))
			{
				qs.set("raids", raids | boss.getMask());
				giveItems(player, LARGE_DRAGON_SKULL, 1);
				if (getQuestItemsCount(player, LARGE_DRAGON_SKULL) < 7)
				{
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
				else
				{
					qs.setCond(2, true);
				}
			}
		}
	}
	
	private static boolean checkMask(QuestState qs, Bosses boss)
	{
		final int pos = boss.getMask();
		return ((qs.getInt("raids") & pos) == pos);
	}
}
