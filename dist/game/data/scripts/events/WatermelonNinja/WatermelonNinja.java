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
package events.WatermelonNinja;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.l2jmobius.commons.util.CommonUtil;
import org.l2jmobius.gameserver.enums.ChatType;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.holders.ItemChanceHolder;
import org.l2jmobius.gameserver.model.quest.LongTimeEvent;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.serverpackets.CreatureSay;

/**
 * @URL https://eu.4gameforum.com/threads/653089/
 * @author Mobius, vGodFather
 */
public class WatermelonNinja extends LongTimeEvent
{
	// NPCs
	private static final int MANAGER = 31860;
	private static final int[] SQUASH_LIST =
	{
		13271,
		13272,
		13273,
		13274,
		13275,
		13276,
		13277,
		13278
	};
	
	// Items
	private static final int[] CHRONO_LIST =
	{
		4202,
		5133,
		5817,
		7058,
		8350
	};
	
	// Skill
	private static final int NECTAR_SKILL = 2005;
	
	// Dialogs.
	private static final String[] NOCHRONO_TEXT =
	{
		"You cannot kill me without Chrono",
		"Hehe... keep trying...",
		"Nice try...",
		"Tired?",
		"Go go! haha..."
	};
	private static final String[] CHRONO_TEXT =
	{
		"Arghh... Chrono weapon...",
		"My end is coming...",
		"Please leave me!",
		"Heeellpppp...",
		"Somebody help me please..."
	};
	private static final String[] NECTAR_TEXT =
	{
		"Yummie... Nectar...",
		"Plase give me more...",
		"Hmmm.. More.. I need more...",
		"I would like you more, if you give me more...",
		"Hmmmmmmm...",
		"My favourite..."
	};
	
	// Rewards.
	private static final Map<Integer, List<ItemChanceHolder>> DROPLIST = new HashMap<>();
	static
	{
		// Defective Watermelon
		final List<ItemChanceHolder> drops13272 = new ArrayList<>();
		drops13272.add(new ItemChanceHolder(29011, 70, 1)); // 1st Class Buff Scroll
		drops13272.add(new ItemChanceHolder(29584, 60, 1)); // Angel Cat's Blessing Chest
		drops13272.add(new ItemChanceHolder(1539, 70, 1)); // Top-grade HP Replenishing Potion
		drops13272.add(new ItemChanceHolder(49080, 60, 1)); // Rice Cake of Fighting Spirit
		drops13272.add(new ItemChanceHolder(29648, 50, 1)); // XP/SP Scroll - Normal
		drops13272.add(new ItemChanceHolder(1870, 50, 1)); // Coal
		drops13272.add(new ItemChanceHolder(1872, 50, 1)); // Animal Bone
		drops13272.add(new ItemChanceHolder(1865, 50, 1)); // Varnish
		drops13272.add(new ItemChanceHolder(2287, 50, 1)); // Recipe: Atuba Hammer (100%)
		drops13272.add(new ItemChanceHolder(2267, 50, 1)); // Recipe: Gastraphetes (100%)
		drops13272.add(new ItemChanceHolder(2276, 50, 1)); // Recipe: Maingauche (100%)
		drops13272.add(new ItemChanceHolder(2289, 50, 1)); // Recipe: Staff of Life (100%)
		drops13272.add(new ItemChanceHolder(2272, 50, 1)); // Recipe: Sword of Revolution (100%)
		drops13272.add(new ItemChanceHolder(2049, 50, 1)); // Atuba Hammer Head
		drops13272.add(new ItemChanceHolder(2029, 50, 1)); // Gastraphetes Shaft
		drops13272.add(new ItemChanceHolder(2038, 50, 1)); // Maingauche Edge
		drops13272.add(new ItemChanceHolder(2051, 50, 1)); // Staff of Life Shaft
		drops13272.add(new ItemChanceHolder(2034, 50, 1)); // Sword of Revolution Blade
		DROPLIST.put(13272, drops13272);
		
		// Prime Watermelon
		final List<ItemChanceHolder> drops13273 = new ArrayList<>();
		drops13273.add(new ItemChanceHolder(29011, 70, 1)); // 1st Class Buff Scroll
		drops13273.add(new ItemChanceHolder(29584, 60, 1)); // Angel Cat's Blessing Chest
		drops13273.add(new ItemChanceHolder(1539, 70, 1)); // Top-grade HP Replenishing Potion
		drops13273.add(new ItemChanceHolder(49080, 60, 1)); // Rice Cake of Fighting Spirit
		drops13273.add(new ItemChanceHolder(29648, 50, 1)); // XP/SP Scroll - Normal
		drops13273.add(new ItemChanceHolder(29519, 40, 1)); // XP/SP Scroll - Medium
		drops13273.add(new ItemChanceHolder(1880, 50, 1)); // Steel
		drops13273.add(new ItemChanceHolder(1877, 50, 1)); // Adamantite Nugget
		drops13273.add(new ItemChanceHolder(1876, 50, 1)); // Mithril Ore
		drops13273.add(new ItemChanceHolder(1882, 50, 1)); // Leather
		drops13273.add(new ItemChanceHolder(1879, 50, 1)); // Cokes
		drops13273.add(new ItemChanceHolder(1881, 50, 1)); // Coarse Bone Powder
		drops13273.add(new ItemChanceHolder(1875, 50, 1)); // Stone of Purity
		drops13273.add(new ItemChanceHolder(2060, 50, 1)); // Stormbringer Blade
		drops13273.add(new ItemChanceHolder(2068, 50, 1)); // Stick of Faith Shaft
		drops13273.add(new ItemChanceHolder(4096, 50, 1)); // Sealed Blue Wolf Glove Fabric
		drops13273.add(new ItemChanceHolder(4090, 50, 1)); // Sealed Blue Wolf Boot Design
		drops13273.add(new ItemChanceHolder(4073, 50, 1)); // Sealed Avadon Glove Fragment
		drops13273.add(new ItemChanceHolder(4098, 50, 1)); // Sealed Avadon Boots Design
		drops13273.add(new ItemChanceHolder(2075, 50, 1)); // Orcish Glaive Blade
		drops13273.add(new ItemChanceHolder(2059, 50, 1)); // Flamberge Blade
		drops13273.add(new ItemChanceHolder(2074, 50, 1)); // Frozen Bow Shaft
		drops13273.add(new ItemChanceHolder(2067, 50, 1)); // Crystal Staff Head
		drops13273.add(new ItemChanceHolder(4080, 50, 1)); // Blue Wolf Gaiter Material
		drops13273.add(new ItemChanceHolder(2063, 50, 1)); // Battle Axe Head
		drops13273.add(new ItemChanceHolder(2301, 50, 1)); // Recipe: Battle Axe (100%)
		drops13273.add(new ItemChanceHolder(4982, 50, 1)); // Recipe: Blue Wolf Gaiters (60%)
		drops13273.add(new ItemChanceHolder(2305, 50, 1)); // Recipe: Crystal Staff (100%)
		drops13273.add(new ItemChanceHolder(2312, 50, 1)); // Recipe: Frozen Bow (100%)
		drops13273.add(new ItemChanceHolder(3017, 50, 1)); // Recipe: Divine Gloves (100%)
		drops13273.add(new ItemChanceHolder(2234, 50, 1)); // Recipe: Divine Stockings (100%)
		drops13273.add(new ItemChanceHolder(2297, 50, 1)); // Recipe: Flamberge (100%)
		drops13273.add(new ItemChanceHolder(3012, 50, 1)); // Recipe: Full Plate Helmet (100%)
		drops13273.add(new ItemChanceHolder(3019, 50, 1)); // Recipe: Full Plate Shield (100%)
		drops13273.add(new ItemChanceHolder(2317, 50, 1)); // Recipe: Bec de Corbin (100%)
		drops13273.add(new ItemChanceHolder(4959, 50, 1)); // Recipe: Sealed Avadon Boots (60%)
		drops13273.add(new ItemChanceHolder(4953, 50, 1)); // Recipe: Sealed Avadon Gloves (60%)
		drops13273.add(new ItemChanceHolder(4992, 50, 1)); // Recipe: Sealed Blue Wolf Boots (60%)
		drops13273.add(new ItemChanceHolder(4998, 50, 1)); // Recipe: Sealed Blue Wolf Gloves (60%)
		drops13273.add(new ItemChanceHolder(2306, 50, 1)); // Recipe: Stick of Faith (100%)
		drops13273.add(new ItemChanceHolder(2298, 50, 1)); // Recipe: Stormbringer (100%)
		DROPLIST.put(13273, drops13273);
		
		// Large Prime Watermelon
		final List<ItemChanceHolder> drops13274 = new ArrayList<>();
		drops13274.add(new ItemChanceHolder(160, 5, 1)); // Battle Axe
		drops13274.add(new ItemChanceHolder(192, 5, 1)); // Crystal Staff
		drops13274.add(new ItemChanceHolder(281, 5, 1)); // Frozen Bow
		drops13274.add(new ItemChanceHolder(71, 5, 1)); // Flamberge
		drops13274.add(new ItemChanceHolder(298, 5, 1)); // Orcish Glaive
		drops13274.add(new ItemChanceHolder(193, 5, 1)); // Stick of Faith
		drops13274.add(new ItemChanceHolder(72, 5, 1)); // Stormbringer
		drops13274.add(new ItemChanceHolder(2463, 5, 1)); // Divine Gloves
		drops13274.add(new ItemChanceHolder(473, 5, 1)); // Divine Stockings
		drops13274.add(new ItemChanceHolder(442, 5, 1)); // Divine Tunic
		drops13274.add(new ItemChanceHolder(401, 5, 1)); // Drake Leather Armor
		drops13274.add(new ItemChanceHolder(2437, 5, 1)); // Drake Leather Boots
		drops13274.add(new ItemChanceHolder(356, 5, 1)); // Full Plate Armor
		drops13274.add(new ItemChanceHolder(2414, 5, 1)); // Full Plate Helmet
		drops13274.add(new ItemChanceHolder(2497, 5, 1)); // Full Plate Shield
		drops13274.add(new ItemChanceHolder(29698, 50, 1)); // 2nd Class Buff Scroll
		drops13274.add(new ItemChanceHolder(29584, 50, 1)); // Angel Cat's Blessing Chest
		drops13274.add(new ItemChanceHolder(1538, 50, 1)); // Improved Scroll of Escape
		drops13274.add(new ItemChanceHolder(3936, 50, 1)); // Blessed Scroll of Resurrection
		drops13274.add(new ItemChanceHolder(5592, 50, 1)); // Greater CP Potion
		drops13274.add(new ItemChanceHolder(1540, 50, 1)); // High-grade HP Recovery Potion
		drops13274.add(new ItemChanceHolder(49081, 50, 1)); // XP Growth Scroll
		drops13274.add(new ItemChanceHolder(49518, 50, 1)); // Special Pirate Fruit
		drops13274.add(new ItemChanceHolder(29010, 50, 1)); // XP/SP Scroll - High
		drops13274.add(new ItemChanceHolder(29519, 50, 1)); // XP/SP Scroll - Medium
		drops13274.add(new ItemChanceHolder(1459, 50, 1)); // C-grade Crystal
		drops13274.add(new ItemChanceHolder(952, 50, 1)); // Scroll: Enchant C-grade Armor
		drops13274.add(new ItemChanceHolder(951, 50, 1)); // Scroll: Enchant C-grade Weapon
		drops13274.add(new ItemChanceHolder(29014, 70, 1)); // Scroll: Dance of Fire
		drops13274.add(new ItemChanceHolder(29013, 70, 1)); // Scroll: Song of Hunter
		drops13274.add(new ItemChanceHolder(1890, 50, 1)); // Mithril Alloy
		drops13274.add(new ItemChanceHolder(4041, 50, 1)); // Mold Hardener
		drops13274.add(new ItemChanceHolder(1893, 50, 1)); // Oriharukon
		drops13274.add(new ItemChanceHolder(1886, 50, 1)); // Silver Mold
		DROPLIST.put(13274, drops13274);
		
		// Defective Honey Watermelon
		final List<ItemChanceHolder> drops13276 = new ArrayList<>();
		drops13276.add(new ItemChanceHolder(187, 20, 1)); // Atuba Hammer
		drops13276.add(new ItemChanceHolder(278, 20, 1)); // Gastraphetes
		drops13276.add(new ItemChanceHolder(224, 20, 1)); // Maingauche
		drops13276.add(new ItemChanceHolder(189, 20, 1)); // Staff of Life
		drops13276.add(new ItemChanceHolder(129, 20, 1)); // Sword of Revolution
		drops13276.add(new ItemChanceHolder(294, 20, 1)); // War Pick
		drops13276.add(new ItemChanceHolder(29011, 50, 1)); // 1st Class Buff Scroll
		drops13276.add(new ItemChanceHolder(29698, 50, 1)); // 2nd Class Buff Scroll
		drops13276.add(new ItemChanceHolder(29584, 50, 1)); // Angel Cat's Blessing Chest
		drops13276.add(new ItemChanceHolder(5592, 50, 1)); // Greater CP Potion
		drops13276.add(new ItemChanceHolder(49080, 50, 1)); // Rice Cake of Fighting Spirit
		drops13276.add(new ItemChanceHolder(49518, 50, 1)); // Special Pirate Fruit
		drops13276.add(new ItemChanceHolder(29010, 50, 1)); // XP/SP Scroll - High
		drops13276.add(new ItemChanceHolder(29519, 50, 1)); // XP/SP Scroll - Medium
		drops13276.add(new ItemChanceHolder(1458, 50, 1)); // D-grade Crystal
		drops13276.add(new ItemChanceHolder(956, 50, 1)); // Scroll: Enchant D-grade Armor
		drops13276.add(new ItemChanceHolder(955, 50, 1)); // Scroll: Enchant D-grade Weapon
		drops13276.add(new ItemChanceHolder(3929, 70, 1)); // Scroll: Acumen
		drops13276.add(new ItemChanceHolder(49435, 70, 1)); // Scroll: Berserker Spirit
		drops13276.add(new ItemChanceHolder(29690, 70, 1)); // Scroll: Blessed Body
		drops13276.add(new ItemChanceHolder(3927, 70, 1)); // Scroll: Death Whisper
		drops13276.add(new ItemChanceHolder(3926, 70, 1)); // Scroll: Guidance
		drops13276.add(new ItemChanceHolder(3930, 70, 1)); // Scroll: Haste
		drops13276.add(new ItemChanceHolder(29689, 70, 1)); // Scroll: Magic Barrier
		drops13276.add(new ItemChanceHolder(4218, 70, 1)); // Scroll: Mana Regeneration
		drops13276.add(new ItemChanceHolder(29688, 70, 1)); // Scroll: Regeneration
		drops13276.add(new ItemChanceHolder(4042, 50, 1)); // Enria
		drops13276.add(new ItemChanceHolder(1890, 50, 1)); // Mithril Alloy
		drops13276.add(new ItemChanceHolder(4041, 50, 1)); // Mold Hardener
		drops13276.add(new ItemChanceHolder(4040, 50, 1)); // Mold Lubricant
		drops13276.add(new ItemChanceHolder(1886, 50, 1)); // Silver Mold
		drops13276.add(new ItemChanceHolder(1887, 50, 1)); // Varnish of Purity
		DROPLIST.put(13276, drops13276);
		
		// Rain Honey Watermelon
		final List<ItemChanceHolder> drops13277 = new ArrayList<>();
		drops13277.add(new ItemChanceHolder(29011, 70, 1)); // 1st Class Buff Scroll
		drops13277.add(new ItemChanceHolder(29584, 60, 1)); // Angel Cat's Blessing Chest
		drops13277.add(new ItemChanceHolder(5592, 60, 1)); // Greater CP Potion
		drops13277.add(new ItemChanceHolder(1540, 60, 1)); // High-grade HP Recovery Potion
		drops13277.add(new ItemChanceHolder(49080, 60, 1)); // Rice Cake of Fighting Spirit
		drops13277.add(new ItemChanceHolder(1877, 50, 1)); // Adamantite Nugget
		drops13277.add(new ItemChanceHolder(4043, 50, 1)); // Asofe
		drops13277.add(new ItemChanceHolder(1881, 50, 1)); // Coarse Bone Powder
		drops13277.add(new ItemChanceHolder(1879, 50, 1)); // Cokes
		drops13277.add(new ItemChanceHolder(1885, 50, 1)); // High-grade Suede
		drops13277.add(new ItemChanceHolder(1876, 50, 1)); // Mithril Ore
		drops13277.add(new ItemChanceHolder(4039, 50, 1)); // Mold Glue
		drops13277.add(new ItemChanceHolder(1874, 50, 1)); // Oriharukon Ore
		drops13277.add(new ItemChanceHolder(1880, 50, 1)); // Steel
		drops13277.add(new ItemChanceHolder(1883, 50, 1)); // Steel Mold
		drops13277.add(new ItemChanceHolder(1875, 50, 1)); // Stone of Purity
		drops13277.add(new ItemChanceHolder(1889, 50, 1)); // Synthetic Braid
		drops13277.add(new ItemChanceHolder(1888, 50, 1)); // Synthetic Cokes
		drops13277.add(new ItemChanceHolder(1887, 50, 1)); // Varnish of Purity
		drops13277.add(new ItemChanceHolder(4071, 50, 1)); // Avadon Robe Fabric
		drops13277.add(new ItemChanceHolder(5530, 50, 1)); // Berserker Blade Edge
		drops13277.add(new ItemChanceHolder(4078, 50, 1)); // Blue Wolf Breastplate Part
		drops13277.add(new ItemChanceHolder(2107, 50, 1)); // Dark Screamer Edge
		drops13277.add(new ItemChanceHolder(1988, 50, 1)); // Divine Tunic Fabric
		drops13277.add(new ItemChanceHolder(2121, 50, 1)); // Eminence Bow Shaft
		drops13277.add(new ItemChanceHolder(2108, 50, 1)); // Fisted Blade Piece
		drops13277.add(new ItemChanceHolder(1986, 50, 1)); // Full Plate Armor Temper
		drops13277.add(new ItemChanceHolder(2093, 50, 1)); // Poleaxe Blade
		drops13277.add(new ItemChanceHolder(2109, 50, 1)); // Akat Longbow Shaft
		drops13277.add(new ItemChanceHolder(4072, 50, 1)); // Sealed Avadon Circlet Pattern
		drops13277.add(new ItemChanceHolder(4088, 50, 1)); // Sealed Blue Wolf Helmet Design
		drops13277.add(new ItemChanceHolder(4089, 50, 1)); // Sealed Doom Helmet Pattern
		drops13277.add(new ItemChanceHolder(2095, 50, 1)); // Sword of Nightmare Blade
		drops13277.add(new ItemChanceHolder(4951, 50, 1)); // Recipe: Avadon Robe (60%)
		drops13277.add(new ItemChanceHolder(5436, 50, 1)); // Recipe: Berserker Blade (100%)
		drops13277.add(new ItemChanceHolder(4981, 50, 1)); // Recipe: Blue Wolf Breastplate (60%)
		drops13277.add(new ItemChanceHolder(2345, 50, 1)); // Recipe: Dark Screamer (100%)
		drops13277.add(new ItemChanceHolder(2233, 50, 1)); // Recipe: Divine Tunic (100%)
		drops13277.add(new ItemChanceHolder(2359, 50, 1)); // Recipe: Eminence Bow (100%)
		drops13277.add(new ItemChanceHolder(2346, 50, 1)); // Recipe: Fisted Blade (100%)
		drops13277.add(new ItemChanceHolder(2231, 50, 1)); // Recipe: Full Plate Armor (100%)
		drops13277.add(new ItemChanceHolder(2330, 50, 1)); // Recipe: Homunkulus' Sword (100%)
		drops13277.add(new ItemChanceHolder(4985, 50, 1)); // Recipe: Doom Leather Armor (60%)
		drops13277.add(new ItemChanceHolder(2331, 50, 1)); // Recipe: Poleaxe (100%)
		drops13277.add(new ItemChanceHolder(2341, 50, 1)); // Recipe: Sage's Staff (100%)
		drops13277.add(new ItemChanceHolder(4952, 50, 1)); // Recipe: Sealed Avadon Circlet (60%)
		drops13277.add(new ItemChanceHolder(4990, 50, 1)); // Recipe: Sealed Blue Wolf Helmet (60%)
		drops13277.add(new ItemChanceHolder(4991, 50, 1)); // Recipe: Sealed Doom Helmet (60%)
		drops13277.add(new ItemChanceHolder(2333, 50, 1)); // Recipe: Sword of Nightmare (100%)
		DROPLIST.put(13277, drops13277);
		
		// Large Rain Honey Watermelon
		final List<ItemChanceHolder> drops13278 = new ArrayList<>();
		drops13278.add(new ItemChanceHolder(5286, 5, 1)); // Berserker Blade
		drops13278.add(new ItemChanceHolder(233, 5, 1)); // Dark Screamer
		drops13278.add(new ItemChanceHolder(286, 5, 1)); // Eminence Bow
		drops13278.add(new ItemChanceHolder(265, 5, 1)); // Fisted Blade
		drops13278.add(new ItemChanceHolder(84, 5, 1)); // Homunkulus' Sword
		drops13278.add(new ItemChanceHolder(95, 5, 1)); // Poleaxe
		drops13278.add(new ItemChanceHolder(200, 5, 1)); // Sage's Staff
		drops13278.add(new ItemChanceHolder(134, 5, 1)); // Sword of Nightmare
		drops13278.add(new ItemChanceHolder(2406, 5, 1)); // Avadon Robe
		drops13278.add(new ItemChanceHolder(358, 5, 1)); // Blue Wolf Breastplate
		drops13278.add(new ItemChanceHolder(2380, 5, 1)); // Blue Wolf Gaiters
		drops13278.add(new ItemChanceHolder(2392, 5, 1)); // Doom Leather Armor
		drops13278.add(new ItemChanceHolder(600, 10, 1)); // Sealed Avadon Boots
		drops13278.add(new ItemChanceHolder(2415, 10, 1)); // Sealed Avadon Circlet
		drops13278.add(new ItemChanceHolder(2464, 10, 1)); // Sealed Avadon Gloves
		drops13278.add(new ItemChanceHolder(2439, 10, 1)); // Sealed Blue Wolf Boots
		drops13278.add(new ItemChanceHolder(2487, 10, 1)); // Sealed Blue Wolf Gloves
		drops13278.add(new ItemChanceHolder(2416, 10, 1)); // Sealed Blue Wolf Helmet
		drops13278.add(new ItemChanceHolder(601, 10, 1)); // Sealed Doom Boots
		drops13278.add(new ItemChanceHolder(2475, 10, 1)); // Sealed Doom Gloves
		drops13278.add(new ItemChanceHolder(2417, 10, 1)); // Sealed Doom Helmet
		drops13278.add(new ItemChanceHolder(29698, 50, 1)); // 2nd Class Buff Scroll
		drops13278.add(new ItemChanceHolder(29584, 50, 1)); // Angel Cat's Blessing Chest
		drops13278.add(new ItemChanceHolder(1538, 50, 1)); // Improved Scroll of Escape
		drops13278.add(new ItemChanceHolder(3936, 50, 1)); // Blessed Scroll of Resurrection
		drops13278.add(new ItemChanceHolder(29022, 30, 1)); // Scroll: Enchant C-grade Armor
		drops13278.add(new ItemChanceHolder(29020, 30, 1)); // Scroll: Enchant D-grade Armor
		drops13278.add(new ItemChanceHolder(29021, 20, 1)); // Scroll: Enchant C-grade Weapon
		drops13278.add(new ItemChanceHolder(29019, 20, 1)); // Scroll: Enchant D-grade Weapon
		drops13278.add(new ItemChanceHolder(1460, 50, 1)); // B-grade Crystal
		drops13278.add(new ItemChanceHolder(1459, 50, 1)); // C-grade Crystal
		drops13278.add(new ItemChanceHolder(5592, 50, 1)); // Greater CP Potion
		drops13278.add(new ItemChanceHolder(1539, 50, 1)); // Top-grade HP Replenishing Potion
		drops13278.add(new ItemChanceHolder(1540, 70, 1)); // High-grade HP Recovery Potion
		drops13278.add(new ItemChanceHolder(49081, 60, 1)); // XP Growth Scroll
		drops13278.add(new ItemChanceHolder(29014, 60, 1)); // Scroll: Dance of Fire
		drops13278.add(new ItemChanceHolder(29013, 60, 1)); // Scroll: Song of Hunter
		drops13278.add(new ItemChanceHolder(952, 50, 1)); // Scroll: Enchant C-grade Armor
		drops13278.add(new ItemChanceHolder(951, 40, 1)); // Scroll: Enchant C-grade Weapon
		drops13278.add(new ItemChanceHolder(49518, 60, 1)); // Special Pirate Fruit
		drops13278.add(new ItemChanceHolder(29010, 60, 1)); // XP/SP Scroll - High
		drops13278.add(new ItemChanceHolder(29519, 60, 1)); // XP/SP Scroll - Medium
		DROPLIST.put(13278, drops13278);
	}
	
	// Drop configuration.
	private static final int MAX_DROP_COUNT = 1;
	
	public WatermelonNinja()
	{
		addAttackId(SQUASH_LIST);
		addKillId(SQUASH_LIST);
		addSpawnId(SQUASH_LIST);
		addSkillSeeId(SQUASH_LIST);
		
		addStartNpc(MANAGER);
		addFirstTalkId(MANAGER);
		addTalkId(MANAGER);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		
		if (event.equals("31860-1.htm"))
		{
			htmltext = event;
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".htm";
	}
	
	@Override
	public String onAttack(Npc npc, Player attacker, int damage, boolean isPet)
	{
		if ((attacker.getActiveWeaponItem() != null) && CommonUtil.contains(CHRONO_LIST, attacker.getActiveWeaponItem().getId()))
		{
			if (getRandom(100) < 20)
			{
				npc.broadcastPacket(new CreatureSay(npc, ChatType.NPC_GENERAL, npc.getName(), getRandomEntry(CHRONO_TEXT)));
			}
			
			npc.setInvul(false);
			npc.setCurrentHp(Math.max(0, npc.getCurrentHp() - 10));
			if (npc.getCurrentHp() <= 1)
			{
				npc.doDie(attacker);
			}
			npc.setInvul(true);
		}
		else if (getRandom(100) < 20)
		{
			npc.broadcastPacket(new CreatureSay(npc, ChatType.NPC_GENERAL, npc.getName(), getRandomEntry(NOCHRONO_TEXT)));
		}
		
		return super.onAttack(npc, attacker, damage, isPet);
	}
	
	@Override
	public String onSkillSee(Npc npc, Player caster, Skill skill, WorldObject[] targets, boolean isPet)
	{
		if ((skill.getId() == NECTAR_SKILL) && (caster.getTarget() == npc))
		{
			switch (npc.getId())
			{
				case 13271: // Watermelon Seed
				{
					randomSpawn(13274, 13273, 13272, npc);
					break;
				}
				case 13275: // Honey Watermelon Seed
				{
					randomSpawn(13278, 13277, 13276, npc);
					break;
				}
			}
		}
		
		return super.onSkillSee(npc, caster, skill, targets, isPet);
	}
	
	@Override
	public String onSpawn(Npc npc)
	{
		npc.setImmobilized(true);
		npc.disableCoreAI(true);
		npc.setInvul(true);
		
		return super.onSpawn(npc);
	}
	
	@Override
	public String onKill(Npc npc, Player killer, boolean isPet)
	{
		final int npcId = npc.getId();
		if (DROPLIST.containsKey(npcId))
		{
			final List<ItemChanceHolder> randomList = new ArrayList<>(DROPLIST.get(npcId));
			Collections.shuffle(randomList);
			
			int dropCount = 0;
			for (ItemChanceHolder drop : randomList)
			{
				if ((dropCount < MAX_DROP_COUNT) && (getRandom(100) < drop.getChance()))
				{
					npc.dropItem(killer, drop);
					dropCount++;
				}
			}
		}
		
		return super.onKill(npc, killer, isPet);
	}
	
	private void randomSpawn(int low, int medium, int high, Npc npc)
	{
		final int npcId = npc.getId();
		if ((npcId == 13274) || (npcId == 13278)) // Fully grown.
		{
			return;
		}
		
		final int random = getRandom(100);
		if (random < 5)
		{
			spawnNext(low, npc);
		}
		else if (random < 10)
		{
			spawnNext(medium, npc);
		}
		else if (random < 30)
		{
			spawnNext(high, npc);
		}
		else if (getRandom(100) < 30)
		{
			npc.broadcastPacket(new CreatureSay(npc, ChatType.NPC_GENERAL, npc.getName(), getRandomEntry(NECTAR_TEXT)));
		}
	}
	
	private void spawnNext(int npcId, Npc npc)
	{
		addSpawn(npcId, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, (npcId == 13274) || (npcId == 13278) ? 30000 : 180000);
		npc.deleteMe();
	}
	
	public static void main(String[] args)
	{
		new WatermelonNinja();
	}
}
