/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package quests.Q00373_SupplierOfReagents;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.enums.QuestSound;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.quest.Quest;
import org.l2jmobius.gameserver.model.quest.QuestState;
import org.l2jmobius.gameserver.model.quest.State;

public class Q00373_SupplierOfReagents extends Quest
{
	// NPCs
	private static final int WESLEY = 30166;
	private static final int URN = 31149;
	// Monsters
	private static final int CRENDION = 20813;
	private static final int HALLATE_MAID = 20822;
	private static final int HALLATE_GUARDIAN = 21061;
	private static final int PLATINUM_TRIBE_SHAMAN = 20828;
	private static final int PLATINUM_GUARDIAN_SHAMAN = 21066;
	private static final int LAVA_WYRM = 21111;
	private static final int HAMES_ORC_SHAMAN = 21115;
	// Quest items
	private static final int MIXING_STONE = 5904;
	private static final int MIXING_MANUAL = 6317;
	// Items - pouches
	private static final int REAGENT_POUCH_1 = 6007;
	private static final int REAGENT_POUCH_2 = 6008;
	private static final int REAGENT_POUCH_3 = 6009;
	private static final int REAGENT_BOX = 6010;
	// Items - ingredients
	private static final int WYRMS_BLOOD = 6011;
	private static final int LAVA_STONE = 6012;
	private static final int MOONSTONE_SHARD = 6013;
	private static final int ROTTEN_BONE = 6014;
	private static final int DEMONS_BLOOD = 6015;
	private static final int INFERNIUM_ORE = 6016;
	// Items - catalysts
	private static final int BLOOD_ROOT = 6017;
	private static final int VOLCANIC_ASH = 6018;
	private static final int QUICKSILVER = 6019;
	private static final int SULFUR = 6020;
	private static final int DEMONIC_ESSENCE = 6031;
	private static final int MIDNIGHT_OIL = 6030;
	// Items - products
	private static final int DRACOPLASM = 6021;
	private static final int MAGMA_DUST = 6022;
	private static final int MOON_DUST = 6023;
	private static final int NECROPLASM = 6024;
	private static final int DEMONPLASM = 6025;
	private static final int INFERNO_DUST = 6026;
	private static final int FIRE_ESSENCE = 6028;
	private static final int LUNARGENT = 6029;
	// Items - products final
	private static final int DRACONIC_ESSENCE = 6027;
	private static final int ABYSS_OIL = 6032;
	private static final int HELLFIRE_OIL = 6033;
	private static final int NIGHTMARE_OIL = 6034;
	private static final int PURE_SILVER = 6320;
	// Variables
	private static final String INGREDIENT = "ingredient";
	private static final String CATALYST = "catalyst";
	
	/**
	 * This droplist defines the npcId, the item dropped and the luck.
	 * <ul>
	 * <li>HAMES_ORC_SHAMAN : 47% chance to drop - reagent pouch (47%)</li>
	 * <li>HALLATES_MAID : 84,4% chance to drop - reageant pouch (66,4%) and volcanic ash (18%)</li>
	 * <li>HALLATES_GUARDIAN : 83,3% chance to drop - demon's blood (72,9%) and moonstone shard (10,4%)</li>
	 * <li>PLATINUM_GUARDIAN_SHAMAN : 44,2% chance to drop - reagent box (44,2%)</li>
	 * <li>PLATINUM_TRIBE_SHAMAN : 100% chance to drop - reagent pouch (68%) and quichsilver (32%)</li>
	 * <li>CRENDION : 100% chance to drop - rotten bone piece (61,8%) and quicksilver (38,2%)</li>
	 * <li>LAVA_WYRM : 75% chance to drop - wyrm's blood (50,5%) and lava stone (24,5%)</li>
	 * </ul>
	 */
	private static final Map<Integer, int[]> DROPLIST = new HashMap<>();
	static
	{
		// @formatter:off
		DROPLIST.put(PLATINUM_GUARDIAN_SHAMAN, new int[]{REAGENT_BOX, 442000, 0});
		DROPLIST.put(HAMES_ORC_SHAMAN, new int[]{REAGENT_POUCH_3, 470000, 0});
		DROPLIST.put(PLATINUM_TRIBE_SHAMAN, new int[]{REAGENT_POUCH_2, QUICKSILVER, 680, 1000});
		DROPLIST.put(HALLATE_MAID, new int[]{REAGENT_POUCH_1, VOLCANIC_ASH, 664, 844});
		DROPLIST.put(HALLATE_GUARDIAN, new int[]{DEMONS_BLOOD, MOONSTONE_SHARD, 729, 833});
		DROPLIST.put(CRENDION, new int[]{ROTTEN_BONE, QUICKSILVER, 618, 1000});
		DROPLIST.put(LAVA_WYRM, new int[]{WYRMS_BLOOD, LAVA_STONE, 505, 750});
		// @formatter:n
	}
	private static final int[][] FORMULAS =
	{
		// @formatter:off
		{10, WYRMS_BLOOD, BLOOD_ROOT, DRACOPLASM},
		{10, LAVA_STONE, VOLCANIC_ASH, MAGMA_DUST},
		{10, MOONSTONE_SHARD, VOLCANIC_ASH, MOON_DUST},
		{10, ROTTEN_BONE, BLOOD_ROOT, NECROPLASM},
		{10, DEMONS_BLOOD, BLOOD_ROOT, DEMONPLASM},
		{10, INFERNIUM_ORE, VOLCANIC_ASH, INFERNO_DUST},
		{10, DRACOPLASM, QUICKSILVER, DRACONIC_ESSENCE},
		{10, MAGMA_DUST, SULFUR, FIRE_ESSENCE},
		{10, MOON_DUST, QUICKSILVER, LUNARGENT},
		{10, NECROPLASM, QUICKSILVER, MIDNIGHT_OIL},
		{10, DEMONPLASM, SULFUR, DEMONIC_ESSENCE},
		{10, INFERNO_DUST, SULFUR, ABYSS_OIL},
		{1, FIRE_ESSENCE, DEMONIC_ESSENCE, HELLFIRE_OIL},
		{1, LUNARGENT, MIDNIGHT_OIL, NIGHTMARE_OIL},
		{1, LUNARGENT, QUICKSILVER, PURE_SILVER}
		// @formatter:on
	};
	private static final int[][] TEMPERATURES =
	{
		// @formatter:off
		{1, 100, 1},
		{2, 45, 2},
		{3, 15, 3}
		// @formatter:on
	};
	
	public Q00373_SupplierOfReagents()
	{
		super(373);
		registerQuestItems(MIXING_STONE, MIXING_MANUAL);
		addStartNpc(WESLEY);
		addTalkId(WESLEY, URN);
		addKillId(CRENDION, HALLATE_MAID, HALLATE_GUARDIAN, PLATINUM_TRIBE_SHAMAN, PLATINUM_GUARDIAN_SHAMAN, LAVA_WYRM, HAMES_ORC_SHAMAN);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		final QuestState st = getQuestState(player, false);
		if (st == null)
		{
			return htmltext;
		}
		
		// Wesley
		if (event.equals("30166-04.htm"))
		{
			st.startQuest();
			giveItems(player, MIXING_STONE, 1);
			giveItems(player, MIXING_MANUAL, 1);
		}
		else if (event.equals("30166-09.htm"))
		{
			st.exitQuest(true, true);
		}
		// Urn
		else if (event.equals("31149-02.htm"))
		{
			if (!hasQuestItems(player, MIXING_STONE))
			{
				htmltext = "31149-04.htm";
			}
		}
		else if (event.startsWith("31149-03-"))
		{
			final int regentId = Integer.parseInt(event.substring(9, 13));
			for (int[] formula : FORMULAS)
			{
				if (formula[1] != regentId)
				{
					continue;
				}
				
				// Not enough items, cancel the operation.
				if (getQuestItemsCount(player, regentId) < formula[0])
				{
					break;
				}
				
				st.set(INGREDIENT, Integer.toString(regentId));
				return htmltext;
			}
			htmltext = "31149-04.htm";
		}
		else if (event.startsWith("31149-06-"))
		{
			final int catalyst = Integer.parseInt(event.substring(9, 13));
			
			// Not enough items, cancel the operation.
			if (!hasQuestItems(player, catalyst))
			{
				return "31149-04.htm";
			}
			
			st.set(CATALYST, Integer.toString(catalyst));
		}
		else if (event.startsWith("31149-12-"))
		{
			final int regent = st.getInt(INGREDIENT);
			final int catalyst = st.getInt(CATALYST);
			for (int[] formula : FORMULAS)
			{
				if ((formula[1] != regent) || (formula[2] != catalyst))
				{
					continue;
				}
				
				// Not enough regents.
				if (getQuestItemsCount(player, regent) < formula[0])
				{
					break;
				}
				
				// Not enough catalysts.
				if (!hasQuestItems(player, catalyst))
				{
					break;
				}
				
				takeItems(player, regent, formula[0]);
				takeItems(player, catalyst, 1);
				
				final int tempIndex = Integer.parseInt(event.substring(9, 10));
				for (int[] temperature : TEMPERATURES)
				{
					if (temperature[0] != tempIndex)
					{
						continue;
					}
					
					if (getRandom(100) < temperature[1])
					{
						giveItems(player, formula[3], temperature[2]);
						return "31149-12-" + formula[3] + ".htm";
					}
					return "31149-11.htm";
				}
			}
			htmltext = "31149-13.htm";
		}
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState st = getQuestState(player, true);
		
		switch (st.getState())
		{
			case State.CREATED:
			{
				htmltext = (player.getLevel() < 57) ? "30166-01.htm" : "30166-02.htm";
				break;
			}
			case State.STARTED:
			{
				if (npc.getId() == WESLEY)
				{
					htmltext = "30166-05.htm";
				}
				else
				{
					htmltext = "31149-01.htm";
				}
				break;
			}
		}
		return htmltext;
	}
	
	@Override
	public String onKill(Npc npc, Player player, boolean isPet)
	{
		final QuestState st = getRandomPartyMemberState(player, -1, 3, npc);
		if (st == null)
		{
			return null;
		}
		final Player partyMember = st.getPlayer();
		
		final int[] drop = DROPLIST.get(npc.getId());
		if (drop[2] == 0)
		{
			if (getRandom(1000000) < drop[1])
			{
				giveItems(partyMember, drop[0], 1);
				playSound(partyMember, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
		else
		{
			final int random = getRandom(1000);
			if (random < drop[3])
			{
				giveItems(partyMember, random < drop[2] ? drop[0] : drop[1], 1);
				playSound(partyMember, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
		
		return null;
	}
}
