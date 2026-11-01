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
package ai.bosses.Tiat;

import java.io.File;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.logging.Logger;

import org.w3c.dom.Document;
import org.w3c.dom.NamedNodeMap;
import org.w3c.dom.Node;

import org.l2jmobius.commons.util.IXmlReader;
import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.managers.GraciaSeedsManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Attackable;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.TrapAction;
import org.l2jmobius.gameserver.model.actor.instance.Door;
import org.l2jmobius.gameserver.model.actor.instance.Trap;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.util.ArrayUtil;
import org.l2jmobius.gameserver.util.PolygonTerritory;

/**
 * Seed of Destruction instance zone.<br>
 * TODO:
 * <ul>
 * <li>No random mob spawns after mob kill.</li>
 * <li>Implement Seed of Destruction Defense state and one party instances.</li>
 * <li>Use proper zone spawn system.</li>
 * </ul>
 * Please maintain consistency between the Seed scripts.
 * @author Gigiikun
 */
public class Stage1 extends InstanceScript implements IXmlReader
{
	private static final Logger LOGGER = Logger.getLogger(Stage1.class.getName());
	
	private static class SODSpawn
	{
		public SODSpawn()
		{
		}
		
		boolean isZone = false;
		boolean isNeededNextFlag = false;
		int npcId;
		int x = 0;
		int y = 0;
		int z = 0;
		int h = 0;
		int zone = 0;
		int count = 0;
	}
	
	// Spawn data
	private final Map<Integer, PolygonTerritory> _spawnZoneList = new HashMap<>();
	private final Map<Integer, List<SODSpawn>> _spawnList = new HashMap<>();
	
	// Locations
	private static final Location ENTER_TELEPORT_2 = new Location(-245800, 220488, -12112);
	private static final Location CENTER_TELEPORT = new Location(-245802, 220528, -12104);
	
	// Traps/Skills
	private static final int[] TRAP_18771_NPCS =
	{
		22541,
		22544,
		22541,
		22544
	};
	private static final int[] TRAP_OTHER_NPCS =
	{
		22546,
		22546,
		22538,
		22537
	};
	
	// NPCs
	private static final int ALENOS = 32526;
	private static final int TELEPORT = 32601;
	
	// Monsters
	private static final int OBELISK = 18776;
	private static final int POWERFUL_DEVICE = 18777;
	private static final int THRONE_POWERFUL_DEVICE = 18778;
	private static final int SPAWN_DEVICE = 18696;
	private static final int TIAT = 29163;
	private static final int TIAT_GUARD = 29162;
	private static final int TIAT_GUARD_NUMBER = 5;
	private static final int TIAT_VIDEO_NPC = 29169;
	private static final Location MOVE_TO_TIAT = new Location(-250403, 207273, -11952, 16384);
	private static final Location MOVE_TO_DOOR = new Location(-251432, 214905, -12088, 16384);
	
	// TODO: handle this better
	private static final int[] SPAWN_MOB_IDS =
	{
		22536,
		22537,
		22538,
		22539,
		22540,
		22541,
		22542,
		22543,
		22544,
		22547,
		22550,
		22551,
		22552,
		22596
	};
	
	// Doors/Walls/Zones
	private static final int[] ATTACKABLE_DOORS =
	{
		12240005,
		12240006,
		12240007,
		12240008,
		12240009,
		12240010,
		12240013,
		12240014,
		12240015,
		12240016,
		12240017,
		12240018,
		12240021,
		12240022,
		12240023,
		12240024,
		12240025,
		12240026,
		12240028,
		12240029,
		12240030
	};
	private static final int[] ENTRANCE_ROOM_DOORS =
	{
		12240001,
		12240002
	};
	private static final int[] SQUARE_DOORS =
	{
		12240003,
		12240004,
		12240011,
		12240012,
		12240019,
		12240020
	};
	private static final int SCOUTPASS_DOOR = 12240027;
	private static final int FORTRESS_DOOR = 12240030;
	private static final int THRONE_DOOR = 12240031;
	
	// Zone
	private static final int VIDEO_ZONE = 60010;
	
	// Misc
	private static final int TEMPLATE_ID = 110; // this is the client number
	private static final int MAX_DEVICESPAWNEDMOBCOUNT = 100; // prevent too much mob spawn
	
	public Stage1()
	{
		super(TEMPLATE_ID);
		load();
		addStartNpc(ALENOS, TELEPORT);
		addTalkId(ALENOS, TELEPORT);
		addAttackId(OBELISK, TIAT);
		addSpawnId(OBELISK, POWERFUL_DEVICE, THRONE_POWERFUL_DEVICE);
		addKillId(OBELISK, TIAT, TIAT_GUARD);
		for (int i = 18771; i <= 18774; i++)
		{
			addTrapActionId(i);
		}
		
		addEnterZoneId(VIDEO_ZONE);
		addInstanceCreatedId(TEMPLATE_ID);
		addDespawnId(SPAWN_DEVICE);
	}
	
	@Override
	public void load()
	{
		parseDatapackFile("data/scripts/ai/bosses/Tiat/SeedOfDestruction.xml");
		LOGGER.info("Seed of Destruction: Loaded " + _spawnZoneList.size() + " spawn zones data.");
	}
	
	@Override
	public void parseDocument(Document document, File file)
	{
		final Set<Integer> killIds = new HashSet<>();
		for (Node n = document.getFirstChild(); n != null; n = n.getNextSibling())
		{
			if (n.getNodeName().equals("list"))
			{
				for (Node d = n.getFirstChild(); d != null; d = d.getNextSibling())
				{
					if (d.getNodeName().equals("npc"))
					{
						for (Node e = d.getFirstChild(); e != null; e = e.getNextSibling())
						{
							if (e.getNodeName().equals("spawn"))
							{
								NamedNodeMap attrs = e.getAttributes();
								final int npcId = parseInteger(attrs, "npcId");
								final int flag = parseInteger(attrs, "flag");
								if (!_spawnList.containsKey(flag))
								{
									_spawnList.put(flag, new ArrayList<>());
								}
								
								for (Node f = e.getFirstChild(); f != null; f = f.getNextSibling())
								{
									if (f.getNodeName().equals("loc"))
									{
										attrs = f.getAttributes();
										
										final SODSpawn spw = new SODSpawn();
										spw.npcId = npcId;
										spw.x = parseInteger(attrs, "x");
										spw.y = parseInteger(attrs, "y");
										spw.z = parseInteger(attrs, "z");
										spw.h = parseInteger(attrs, "heading");
										spw.isNeededNextFlag = parseBoolean(attrs, "mustKill", false);
										if (spw.isNeededNextFlag)
										{
											killIds.add(npcId);
										}
										
										_spawnList.get(flag).add(spw);
									}
									else if (f.getNodeName().equals("zone"))
									{
										attrs = f.getAttributes();
										
										final SODSpawn spw = new SODSpawn();
										spw.npcId = npcId;
										spw.isZone = true;
										spw.zone = parseInteger(attrs, "id");
										spw.count = parseInteger(attrs, "count");
										spw.isNeededNextFlag = parseBoolean(attrs, "mustKill", false);
										if (spw.isNeededNextFlag)
										{
											killIds.add(npcId);
										}
										
										_spawnList.get(flag).add(spw);
									}
								}
							}
						}
					}
					else if (d.getNodeName().equals("spawnZones"))
					{
						for (Node e = d.getFirstChild(); e != null; e = e.getNextSibling())
						{
							if (e.getNodeName().equals("zone"))
							{
								NamedNodeMap attrs = e.getAttributes();
								final int id = parseInteger(attrs, "id");
								final int minz = parseInteger(attrs, "minZ");
								final int maxz = parseInteger(attrs, "maxZ");
								final PolygonTerritory ter = new PolygonTerritory(id);
								for (Node f = e.getFirstChild(); f != null; f = f.getNextSibling())
								{
									if (f.getNodeName().equals("point"))
									{
										attrs = f.getAttributes();
										final int x = parseInteger(attrs, "x");
										final int y = parseInteger(attrs, "y");
										ter.add(x, y, minz, maxz, 0);
									}
								}
								
								_spawnZoneList.put(id, ter);
							}
						}
					}
				}
			}
		}
		
		addKillId(killIds);
	}
	
	@Override
	public void onInstanceCreated(Instance instance, Player player)
	{
		spawnState(instance);
		for (Door door : instance.getDoors())
		{
			if (ArrayUtil.contains(ATTACKABLE_DOORS, door.getId()))
			{
				door.setIsAttackableDoor(true);
			}
			
			door.closeMe();
		}
	}
	
	protected boolean checkKillProgress(Instance world)
	{
		return world.getNpcs().stream().filter(n -> !n.isDead() && n.isScriptValue(1)).count() == 0;
	}
	
	private void spawnFlaggedNPCs(Instance world, int flag)
	{
		for (SODSpawn spw : _spawnList.get(flag))
		{
			if (spw.isZone)
			{
				if (_spawnZoneList.containsKey(spw.zone))
				{
					final PolygonTerritory terr = _spawnZoneList.get(spw.zone);
					for (int i = 0; i < spw.count; i++)
					{
						final Location location = terr.getRandomPoint();
						if (location != null)
						{
							spawn(world, spw.npcId, location.getX(), location.getY(), GeoEngine.getInstance().getHeight(location.getX(), location.getY(), location.getZ()), getRandom(65535), spw.isNeededNextFlag);
						}
					}
				}
				else
				{
					LOGGER.info("Seed of Destruction: Missing zone " + spw.zone);
				}
			}
			else
			{
				spawn(world, spw.npcId, spw.x, spw.y, spw.z, spw.h, spw.isNeededNextFlag);
			}
		}
	}
	
	protected void spawnState(Instance world)
	{
		world.incStatus();
		world.getAliveNpcs().forEach(n -> n.setScriptValue(0));
		switch (world.getStatus() - 1)
		{
			case 0:
			{
				spawnFlaggedNPCs(world, 0);
				break;
			}
			case 1:
			{
				world.broadcastPacket(new ExShowScreenMessage(NpcStringId.THE_ENEMIES_HAVE_ATTACKED_EVERYONE_COME_OUT_AND_FIGHT_URGH, 5, 1000));
				for (int i : ENTRANCE_ROOM_DOORS)
				{
					world.openCloseDoor(i, true);
				}
				
				spawnFlaggedNPCs(world, 1);
				break;
			}
			case 4:
			{
				world.broadcastPacket(new ExShowScreenMessage(NpcStringId.OBELISK_HAS_COLLAPSED_DON_T_LET_THE_ENEMIES_JUMP_AROUND_WILDLY_ANYMORE, 5, 1000));
				for (int i : SQUARE_DOORS)
				{
					world.openCloseDoor(i, true);
				}
				
				spawnFlaggedNPCs(world, 4);
				break;
			}
			case 5:
			{
				world.openCloseDoor(SCOUTPASS_DOOR, true);
				spawnFlaggedNPCs(world, 3);
				spawnFlaggedNPCs(world, 5);
				break;
			}
			case 6:
			{
				world.openCloseDoor(THRONE_DOOR, true);
				break;
			}
			case 7:
			{
				spawnFlaggedNPCs(world, 7);
				break;
			}
			case 8:
			{
				world.broadcastPacket(new ExShowScreenMessage(NpcStringId.COME_OUT_WARRIORS_PROTECT_SEED_OF_DESTRUCTION, 5, 1000));
				world.setParameter("deviceCount", 0);
				spawnFlaggedNPCs(world, 8);
				break;
			}
		}
	}
	
	protected void spawn(Instance world, int npcId, int x, int y, int z, int h, boolean addToKillTable)
	{
		// traps
		if ((npcId >= 18720) && (npcId <= 18774))
		{
			addTrap(npcId, x, y, z, h, world.getId());
			return;
		}
		
		final Npc npc = addSpawn(npcId, x, y, z, h, false, 0, false, world.getId());
		if (addToKillTable)
		{
			npc.setScriptValue(1);
		}
		
		if (npc.isAttackable())
		{
			npc.asAttackable().setSeeThroughSilentMove(true);
		}
		
		if (npcId == TIAT_VIDEO_NPC)
		{
			startQuestTimer("DoorCheck", 10000, npc, null);
		}
		else if (npcId == SPAWN_DEVICE)
		{
			npc.disableCoreAI(true);
			startQuestTimer("Spawn", 10000, npc, null, true);
		}
		else if (npcId == TIAT)
		{
			for (int i = 0; i < TIAT_GUARD_NUMBER; i++)
			{
				addMinion(npc.asMonster(), TIAT_GUARD);
			}
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.disableCoreAI(true);
	}
	
	@Override
	public void onEnterZone(Creature creature, ZoneType zone)
	{
		if (creature.isPlayer())
		{
			final Instance world = creature.getInstanceWorld();
			if ((world != null) && world.isStatus(7))
			{
				spawnState(world);
				final Npc videoNpc = world.getNpc(TIAT_VIDEO_NPC);
				if (videoNpc != null)
				{
					playMovie(World.getInstance().getVisibleObjectsInRange(videoNpc, Player.class, 8000), Movie.SC_BOSS_TIAT_OPENING);
					videoNpc.deleteMe();
				}
			}
		}
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon, Skill skill)
	{
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			if (npc.getId() == OBELISK)
			{
				if (world.isStatus(2))
				{
					world.setStatus(4);
					spawnFlaggedNPCs(world, 3);
				}
				else if (world.isStatus(3))
				{
					world.setStatus(4);
					spawnFlaggedNPCs(world, 2);
				}
			}
			else if ((world.getStatus() <= 8) && (npc.getCurrentHp() < (npc.getMaxHp() / 2)))
			{
				spawnState(world);
				startQuestTimer("TiatFullHp", 3000, npc, null);
				world.setReenterTime();
			}
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			switch (event)
			{
				case "Spawn":
				{
					final List<Player> players = new ArrayList<>(world.getPlayers());
					final int deviceCount = world.getParameters().getInt("deviceCount", 0);
					if ((deviceCount < MAX_DEVICESPAWNEDMOBCOUNT) && !players.isEmpty() && !getRandomEntry(players).isDead())
					{
						world.setParameter("deviceCount", deviceCount + 1);
						
						final Attackable mob = addSpawn(getRandomEntry(SPAWN_MOB_IDS), npc.getSpawn().getLocation(), false, 0, false, world.getId()).asAttackable();
						mob.setSeeThroughSilentMove(true);
						mob.setRunning();
						mob.getAI().setIntention(Intention.MOVE_TO, (world.getStatus() >= 7) ? MOVE_TO_TIAT : MOVE_TO_DOOR);
					}
					break;
				}
				case "DoorCheck":
				{
					final Door tmp = world.getDoor(FORTRESS_DOOR);
					if (tmp.getCurrentHp() < tmp.getMaxHp())
					{
						world.setParameter("deviceCount", 0);
						spawnFlaggedNPCs(world, 6);
						world.broadcastPacket(new ExShowScreenMessage(NpcStringId.ENEMIES_ARE_TRYING_TO_DESTROY_THE_FORTRESS_EVERYONE_DEFEND_THE_FORTRESS, ExShowScreenMessage.MIDDLE_CENTER, 1000));
					}
					else
					{
						startQuestTimer("DoorCheck", 10000, npc, null);
					}
					break;
				}
				case "TiatFullHp":
				{
					if (!npc.hasBlockActions() && !npc.isHpBlocked())
					{
						npc.setCurrentHp(npc.getMaxHp());
					}
					break;
				}
			}
		}
		
		return null;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			switch (world.getStatus())
			{
				case 1:
				{
					if (checkKillProgress(world))
					{
						spawnState(world);
					}
					break;
				}
				case 2:
				{
					if (checkKillProgress(world))
					{
						world.incStatus();
					}
					break;
				}
				case 4:
				{
					if (npc.getId() == OBELISK)
					{
						spawnState(world);
					}
					break;
				}
				case 5:
				{
					if ((npc.getId() == POWERFUL_DEVICE) && checkKillProgress(world))
					{
						spawnState(world);
					}
					break;
				}
				case 6:
				{
					if ((npc.getId() == THRONE_POWERFUL_DEVICE) && checkKillProgress(world))
					{
						spawnState(world);
					}
					break;
				}
				default:
				{
					if (world.getStatus() >= 7)
					{
						if (npc.getId() == TIAT)
						{
							world.incStatus();
							playMovie(World.getInstance().getVisibleObjectsInRange(npc, Player.class, 8000), Movie.SC_BOSS_TIAT_ENDING_SUCCES);
							world.removeNpcs();
							world.finishInstance();
							GraciaSeedsManager.getInstance().increaseSoDTiatKilled();
						}
						else if (npc.getId() == TIAT_GUARD)
						{
							addMinion(npc.asMonster().getLeader(), TIAT_GUARD);
						}
					}
				}
			}
		}
	}
	
	@Override
	public void onNpcDespawn(Npc npc)
	{
		cancelQuestTimer("Spawn", npc, null);
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final int npcId = npc.getId();
		if (npcId == ALENOS)
		{
			final int state = GraciaSeedsManager.getInstance().getSoDState();
			if (state == 1)
			{
				enterInstance(player, npc, TEMPLATE_ID);
			}
			else if (state == 2)
			{
				player.teleToLocation(ENTER_TELEPORT_2);
			}
		}
		else
		{
			player.teleToLocation(CENTER_TELEPORT);
		}
		
		return null;
	}
	
	@Override
	public void onTrapAction(Trap trap, Creature trigger, TrapAction action)
	{
		final Instance world = trap.getInstanceWorld();
		if ((world != null) && (action == TrapAction.TRAP_TRIGGERED))
		{
			final int[] npcs = (trap.getId() == 18771) ? TRAP_18771_NPCS : TRAP_OTHER_NPCS;
			for (int npcId : npcs)
			{
				addSpawn(npcId, trap.getX(), trap.getY(), trap.getZ(), trap.getHeading(), true, 0, true, world.getId());
			}
		}
	}
	
	public static void main(String[] args)
	{
		new Stage1();
	}
}
