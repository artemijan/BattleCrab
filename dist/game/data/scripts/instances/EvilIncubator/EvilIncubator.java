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
package instances.EvilIncubator;

import java.util.EnumMap;
import java.util.List;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Attackable;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.actor.instance.FriendlyNpc;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureDeath;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.Earthquake;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10341_DayOfDestinyHumansFate.Q10341_DayOfDestinyHumansFate;
import quests.Q10342_DayOfDestinyElvenFate.Q10342_DayOfDestinyElvenFate;
import quests.Q10343_DayOfDestinyDarkElfsFate.Q10343_DayOfDestinyDarkElfsFate;
import quests.Q10344_DayOfDestinyOrcsFate.Q10344_DayOfDestinyOrcsFate;
import quests.Q10345_DayOfDestinyDwarfsFate.Q10345_DayOfDestinyDwarfsFate;
import quests.Q10346_DayOfDestinyKamaelsFate.Q10346_DayOfDestinyKamaelsFate;

/**
 * Evil Incubator instance zone.
 * @author St3eT
 */
public class EvilIncubator extends InstanceScript
{
	// NPCs
	private static final int VANGUARD_MEMBER = 33165;
	private static final int ADOLPH = 33170;
	private static final int ALICE = 33171;
	private static final int BARTON = 33172;
	private static final int HAYUK = 33173;
	private static final int ELIYAH = 33174;
	private static final int[] MONSTERS =
	{
		27431, // Slayer
		27432, // Tracker
		27433, // Priest of Darkness
		27434, // Guardian of Darkness
		27430, // Screaming Shaman
	};
	private static final int[] BOSSES =
	{
		27425, // Death Wound
		27426, // Death Wound
		27427, // Death Wound
		27428, // Death Wound
		27429, // Death Wound
	};
	private static final int[] HELPERS =
	{
		33170, // Adolph
		// 33171, // Priest Alice
		33172, // Officer Barton
		33173, // Sniper Hayuk
		33174, // Magus Eliyah
		33414, // Van Archer
		33415, // Van Infantry
	};
	
	// Items
	private static final int SOE = 736; // Scroll of Escape
	
	// Locations
	private static final Location PLAYER_BATTLE_LOC = new Location(56167, -175615, -7944);
	private static final Location BATTLE_LOC_2 = new Location(56005, -175536, -7952);
	private static final Location BATTLE_LOC_3 = new Location(56325, -175536, -7952);
	private static final Location BOSS_LOC = new Location(56165, -177550, -7944);
	
	// Misc
	private static final int TEMPLATE_ID = 185;
	// @formatter:off
	private static final EnumMap<Race, String> RACE_QUESTS = new EnumMap<>(Race.class);
	static
	{
		RACE_QUESTS.put(Race.HUMAN, Q10341_DayOfDestinyHumansFate.class.getSimpleName());
		RACE_QUESTS.put(Race.ELF, Q10342_DayOfDestinyElvenFate.class.getSimpleName());
		RACE_QUESTS.put(Race.DARK_ELF, Q10343_DayOfDestinyDarkElfsFate.class.getSimpleName());
		RACE_QUESTS.put(Race.ORC, Q10344_DayOfDestinyOrcsFate.class.getSimpleName());
		RACE_QUESTS.put(Race.DWARF, Q10345_DayOfDestinyDwarfsFate.class.getSimpleName());
		RACE_QUESTS.put(Race.KAMAEL, Q10346_DayOfDestinyKamaelsFate.class.getSimpleName());
	}
	private static final EnumMap<PlayerClass, Integer> CLASS_BOSS = new EnumMap<>(PlayerClass.class);
	static
	{
		CLASS_BOSS.put(PlayerClass.GLADIATOR, 27426);
		CLASS_BOSS.put(PlayerClass.WARLORD, 27426);
		CLASS_BOSS.put(PlayerClass.PALADIN, 27425);
		CLASS_BOSS.put(PlayerClass.DARK_AVENGER, 27425);
		CLASS_BOSS.put(PlayerClass.TREASURE_HUNTER, 27426);
		CLASS_BOSS.put(PlayerClass.HAWKEYE, 27428);
		CLASS_BOSS.put(PlayerClass.SORCERER, 27428);
		CLASS_BOSS.put(PlayerClass.NECROMANCER, 27428);
		CLASS_BOSS.put(PlayerClass.WARLOCK, 27428);
		CLASS_BOSS.put(PlayerClass.BISHOP, 27427);
		CLASS_BOSS.put(PlayerClass.PROPHET, 27429);
		CLASS_BOSS.put(PlayerClass.TEMPLE_KNIGHT, 27425);
		CLASS_BOSS.put(PlayerClass.SWORDSINGER, 27429);
		CLASS_BOSS.put(PlayerClass.PLAINS_WALKER, 27426);
		CLASS_BOSS.put(PlayerClass.SILVER_RANGER, 27428);
		CLASS_BOSS.put(PlayerClass.SPELLSINGER, 27428);
		CLASS_BOSS.put(PlayerClass.ELEMENTAL_SUMMONER, 27428);
		CLASS_BOSS.put(PlayerClass.ELDER, 27427);
		CLASS_BOSS.put(PlayerClass.SHILLIEN_KNIGHT, 27425);
		CLASS_BOSS.put(PlayerClass.BLADEDANCER, 27429);
		CLASS_BOSS.put(PlayerClass.ABYSS_WALKER, 27426);
		CLASS_BOSS.put(PlayerClass.PHANTOM_RANGER, 27428);
		CLASS_BOSS.put(PlayerClass.SPELLHOWLER, 27428);
		CLASS_BOSS.put(PlayerClass.PHANTOM_SUMMONER, 27428);
		CLASS_BOSS.put(PlayerClass.SHILLIEN_ELDER, 27427);
		CLASS_BOSS.put(PlayerClass.DESTROYER, 27426);
		CLASS_BOSS.put(PlayerClass.TYRANT, 27426);
		CLASS_BOSS.put(PlayerClass.OVERLORD, 27429);
		CLASS_BOSS.put(PlayerClass.WARCRYER, 27429);
		CLASS_BOSS.put(PlayerClass.BOUNTY_HUNTER, 27426);
		CLASS_BOSS.put(PlayerClass.WARSMITH, 27426);
		CLASS_BOSS.put(PlayerClass.BERSERKER, 27426);
		CLASS_BOSS.put(PlayerClass.MALE_SOULBREAKER, 27426);
		CLASS_BOSS.put(PlayerClass.FEMALE_SOULBREAKER, 27426);
		CLASS_BOSS.put(PlayerClass.ARBALESTER, 27428);
	}
	private static final EnumMap<PlayerClass, Integer> CLASS_ITEMS = new EnumMap<>(PlayerClass.class);
	static
	{
		CLASS_ITEMS.put(PlayerClass.GLADIATOR, 17484);
		CLASS_ITEMS.put(PlayerClass.WARLORD, 17485);
		CLASS_ITEMS.put(PlayerClass.PALADIN, 17486);
		CLASS_ITEMS.put(PlayerClass.DARK_AVENGER, 17487);
		CLASS_ITEMS.put(PlayerClass.TREASURE_HUNTER, 17488);
		CLASS_ITEMS.put(PlayerClass.HAWKEYE, 17489);
		CLASS_ITEMS.put(PlayerClass.SORCERER, 17490);
		CLASS_ITEMS.put(PlayerClass.NECROMANCER, 17491);
		CLASS_ITEMS.put(PlayerClass.WARLOCK, 17492);
		CLASS_ITEMS.put(PlayerClass.BISHOP, 17493);
		CLASS_ITEMS.put(PlayerClass.PROPHET, 17494);
		CLASS_ITEMS.put(PlayerClass.TEMPLE_KNIGHT, 17495);
		CLASS_ITEMS.put(PlayerClass.SWORDSINGER, 17496);
		CLASS_ITEMS.put(PlayerClass.PLAINS_WALKER, 17497);
		CLASS_ITEMS.put(PlayerClass.SILVER_RANGER, 17498);
		CLASS_ITEMS.put(PlayerClass.SPELLSINGER, 17499);
		CLASS_ITEMS.put(PlayerClass.ELEMENTAL_SUMMONER, 17500);
		CLASS_ITEMS.put(PlayerClass.ELDER, 17501);
		CLASS_ITEMS.put(PlayerClass.SHILLIEN_KNIGHT, 17502);
		CLASS_ITEMS.put(PlayerClass.BLADEDANCER, 17503);
		CLASS_ITEMS.put(PlayerClass.ABYSS_WALKER, 17504);
		CLASS_ITEMS.put(PlayerClass.PHANTOM_RANGER, 17505);
		CLASS_ITEMS.put(PlayerClass.SPELLHOWLER, 17506);
		CLASS_ITEMS.put(PlayerClass.PHANTOM_SUMMONER, 17507);
		CLASS_ITEMS.put(PlayerClass.SHILLIEN_ELDER, 17508);
		CLASS_ITEMS.put(PlayerClass.DESTROYER, 17509);
		CLASS_ITEMS.put(PlayerClass.TYRANT, 17510);
		CLASS_ITEMS.put(PlayerClass.OVERLORD, 17511);
		CLASS_ITEMS.put(PlayerClass.WARCRYER, 17512);
		CLASS_ITEMS.put(PlayerClass.BOUNTY_HUNTER, 17513);
		CLASS_ITEMS.put(PlayerClass.WARSMITH, 17514);
		CLASS_ITEMS.put(PlayerClass.BERSERKER, 17515);
		CLASS_ITEMS.put(PlayerClass.MALE_SOULBREAKER, 17516);
		CLASS_ITEMS.put(PlayerClass.FEMALE_SOULBREAKER, 17516);
		CLASS_ITEMS.put(PlayerClass.ARBALESTER, 17517);
	}
	// @formatter:on
	
	public EvilIncubator()
	{
		super(TEMPLATE_ID);
		addStartNpc(VANGUARD_MEMBER);
		addTalkId(VANGUARD_MEMBER, ADOLPH, ALICE, BARTON, HAYUK, ELIYAH);
		addFirstTalkId(ADOLPH, ALICE, BARTON, HAYUK, ELIYAH);
		setCreatureKillId(this::onCreatureKill, MONSTERS);
		setCreatureKillId(this::onCreatureKill, BOSSES);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState st = getQuestState(player);
		if ((st == null) || !st.isStarted())
		{
			return super.onEvent(event, npc, player);
		}
		
		String htmltext = null;
		if (event.equals("enterInstance"))
		{
			enterInstance(player, npc, TEMPLATE_ID);
			st.setCond(5);
			htmltext = "33165-01.html";
		}
		else
		{
			final Instance world = npc.getInstanceWorld();
			if (world != null)
			{
				switch (event)
				{
					case "33170-02.html":
					{
						if (st.isCond(5))
						{
							st.setCond(6, true);
							htmltext = event;
						}
						break;
					}
					case "selectHelper":
					{
						int helperCount = world.getParameters().getInt("HELPER_COUNT", 0);
						if ((helperCount < 2) && st.isCond(6))
						{
							helperCount++;
							world.setParameter("HELPER_COUNT", helperCount);
							npc.teleToLocation(helperCount == 1 ? BATTLE_LOC_2 : BATTLE_LOC_3);
							if (helperCount == 2)
							{
								st.setCond(7, true);
								World.getInstance().getVisibleObjectsInRange(world.getNpc(ADOLPH), FriendlyNpc.class, 1000).forEach(FriendlyNpc::deleteMe);
							}
						}
						break;
					}
					case "teamSelected":
					{
						int helperCount = world.getParameters().getInt("HELPER_COUNT", 0);
						if ((helperCount == 2) && st.isCond(7))
						{
							helperCount++;
							world.setParameter("HELPER_COUNT", helperCount);
							st.setCond(8, true);
							npc.teleToLocation(PLAYER_BATTLE_LOC);
							player.teleToLocation(PLAYER_BATTLE_LOC);
							getTimers().addTimer("SPAWN_WAVE", 5000, e -> manageWaveSpawn(world));
						}
						break;
					}
					case "giveItem":
					{
						if (st.isCond(9))
						{
							st.setCond(10, true);
							if (CLASS_ITEMS.containsKey(player.getPlayerClass()))
							{
								final int classItemId = CLASS_ITEMS.get(player.getPlayerClass()).intValue();
								if (!hasQuestItems(player, classItemId))
								{
									giveItems(player, classItemId, 1);
								}
							}
							
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THE_CRY_OF_DESTINY_PENDANT_WILL_BE_HELPFUL_TO_YOU_PLEASE_EQUIP_IT_AND_BRING_OUT_THE_POWER_OF_THE_PENDANT_TO_PREPARE_FOR_THE_NEXT_FIGHT);
							htmltext = "33170-06.html";
						}
						break;
					}
					case "resumeFight":
					{
						if (st.isCond(10))
						{
							st.setCond(11, true);
							manageWaveSpawn(world);
						}
						break;
					}
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState st = getQuestState(player);
		if ((st == null) || !st.isStarted())
		{
			return super.onTalk(npc, player);
		}
		
		if (st.getState() == State.STARTED)
		{
			switch (npc.getId())
			{
				case ADOLPH:
				{
					switch (st.getCond())
					{
						case 5:
						{
							htmltext = npc.getId() + "-01.html";
							break;
						}
						case 6:
						{
							htmltext = "33170-02.html";
							break;
						}
						case 7:
						{
							htmltext = "33170-03.html";
							break;
						}
						case 9:
						{
							htmltext = "33170-05.html";
							break;
						}
						case 10:
						{
							htmltext = "33170-07.html";
							break;
						}
						case 12:
						{
							st.setCond(13, true);
							giveItems(player, SOE, 1);
							htmltext = "33170-08.html";
							break;
						}
						case 13:
						{
							htmltext = "33170-08.html";
							break;
						}
					}
					break;
				}
				case ALICE:
				case BARTON:
				case ELIYAH:
				case HAYUK:
				{
					switch (st.getCond())
					{
						case 5:
						{
							htmltext = npc.getId() + "-01.html";
							break;
						}
						case 6:
						{
							htmltext = npc.getId() + "-02.html";
							break;
						}
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState st = getQuestState(player);
		if ((st == null) || !st.isStarted())
		{
			return htmltext;
		}
		
		switch (npc.getId())
		{
			case ADOLPH:
			{
				switch (st.getCond())
				{
					case 5:
					case 6:
					case 7:
					case 9:
					case 10:
					case 12:
					case 13:
					{
						htmltext = "33170.html";
						break;
					}
					default:
					{
						htmltext = "33170-04.html";
					}
				}
				break;
			}
			case ALICE:
			case BARTON:
			case ELIYAH:
			case HAYUK:
			{
				htmltext = st.getCond() > 7 ? (npc.getId() + "-03.html") : (npc.getId() + ".html");
				break;
			}
		}
		
		return htmltext;
	}
	
	public void onCreatureKill(OnCreatureDeath event)
	{
		final Npc npc = event.getTarget().asNpc();
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			final int waveId = world.getParameters().getInt("WORLD_WAVE", 1);
			if (ArrayUtil.contains(BOSSES, npc.getId()))
			{
				final QuestState st = getQuestState(world.getFirstPlayer());
				if ((st == null) || !st.isStarted())
				{
					return;
				}
				
				st.setCond(12, true);
			}
			else if ((waveId < 8) && (world.getAliveNpcCount(MONSTERS) == 0))
			{
				getTimers().addTimer("SPAWN_WAVE", 5000, e -> manageWaveSpawn(world));
			}
			
			getTimers().addTimer("WORLD_ATTACK", 1000, e -> managerWorldAttack(world, null));
		}
	}
	
	private void manageWaveSpawn(Instance world)
	{
		final QuestState st = getQuestState(world.getFirstPlayer());
		if ((st == null) || !st.isStarted())
		{
			return;
		}
		
		List<Npc> spawnedNpcs = null;
		int waveId = world.getParameters().getInt("WORLD_WAVE", 1);
		switch (waveId)
		{
			case 1:
			case 2:
			case 3:
			case 4:
			case 5:
			case 6:
			{
				spawnedNpcs = world.spawnGroup("wave_" + waveId);
				waveId++;
				world.setParameter("WORLD_WAVE", waveId);
				showOnScreenMsg(world, NpcStringId.CREATURES_RESURRECTED_DEFEND_YOURSELF, ExShowScreenMessage.TOP_CENTER, 5000, true);
				break;
			}
			case 7:
			{
				waveId++;
				world.setParameter("WORLD_WAVE", waveId);
				showOnScreenMsg(world, NpcStringId.CREATURES_HAVE_STOPPED_THEIR_ATTACK_REST_AND_THEN_SPEAK_WITH_ADOLPH, ExShowScreenMessage.TOP_CENTER, 5000, true);
				st.setCond(9, true);
				break;
			}
			case 8:
			case 9:
			case 10:
			case 11:
			case 12:
			case 13:
			case 14:
			{
				spawnedNpcs = world.spawnGroup("wave_" + waveId);
				waveId++;
				world.setParameter("WORLD_WAVE", waveId);
				showOnScreenMsg(world, NpcStringId.CREATURES_RESURRECTED_DEFEND_YOURSELF, ExShowScreenMessage.TOP_CENTER, 5000, true);
				getTimers().addTimer("SPAWN_WAVE", 20000, e -> manageWaveSpawn(world));
				break;
			}
			case 15:
			{
				spawnedNpcs = world.spawnGroup("wave_" + waveId);
				waveId++;
				world.setParameter("WORLD_WAVE", waveId);
				showOnScreenMsg(world, NpcStringId.I_DEATH_WOUND_CHAMPION_OF_SHILEN_SHALL_END_YOUR_WORLD, ExShowScreenMessage.TOP_CENTER, 5000, true);
				getTimers().addTimer("SPAWN_WAVE", 20000, e -> manageWaveSpawn(world));
				final Npc boss = addSpawn(CLASS_BOSS.get(world.getFirstPlayer().getPlayerClass()).intValue(), BOSS_LOC, false, 0, false, world.getId());
				world.broadcastPacket(new Earthquake(boss, 50, 10));
				break;
			}
		}
		
		managerWorldAttack(world, spawnedNpcs);
	}
	
	private void managerWorldAttack(Instance world, List<Npc> spawnedNpcs)
	{
		final List<FriendlyNpc> helperList = World.getInstance().getVisibleObjects(world.getFirstPlayer(), FriendlyNpc.class);
		if ((spawnedNpcs != null) && !spawnedNpcs.isEmpty())
		{
			for (Npc npc : spawnedNpcs)
			{
				if (!helperList.isEmpty())
				{
					final FriendlyNpc helper = helperList.get(getRandom(helperList.size()));
					if (ArrayUtil.contains(HELPERS, helper.getId()))
					{
						npc.reduceCurrentHp(1, helper, null);
						helper.reduceCurrentHp(1, npc, null);
						addAttackDesire(helper, npc);
						helperList.remove(helper);
					}
				}
			}
		}
		
		for (FriendlyNpc helper : helperList)
		{
			for (Attackable monster : World.getInstance().getVisibleObjects(helper, Attackable.class))
			{
				if (!(monster instanceof FriendlyNpc))
				{
					addAttackDesire(helper, monster);
				}
			}
		}
	}
	
	private QuestState getQuestState(Player player)
	{
		if (player == null)
		{
			return null;
		}
		
		return RACE_QUESTS.containsKey(player.getRace()) ? player.getQuestState(RACE_QUESTS.get(player.getRace())) : null;
	}
	
	public static void main(String[] args)
	{
		new EvilIncubator();
	}
}
