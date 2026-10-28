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
package ai.bosses.Helios;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ScheduledFuture;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.config.GrandBossConfig;
import org.l2jmobius.gameserver.data.holders.SpawnHolder;
import org.l2jmobius.gameserver.managers.GrandBossManager;
import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.GrandBoss;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.zone.type.NoSummonFriendZone;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.util.Broadcast;

/**
 * @author Mobius, NviX
 */
public class Helios extends Script
{
	// Raid
	private static final int HELIOS1 = 29303;
	private static final int HELIOS2 = 29304;
	private static final int HELIOS3 = 29305;
	
	// Minions
	private static final int LEOPOLD = 29306;
	private static final int HELIOS_RED_LIGHTNING = 29307;
	private static final int HELIOS_BLUE_LIGHTNING = 29308;
	private static final int LEOPOLD_ORIGIN = 29309;
	private static final int ENUMA_ELISH_ORIGIN = 29310;
	private static final int ROYAL_TEMPLAR_COLONEL = 29311;
	private static final int ROYAL_SHARPSHOOTER = 29312;
	private static final int ROYAL_ARCHMAGE = 29313;
	private static final int ROYAL_GATEKEEPER = 29314;
	private static final int MIMILLION = 29315;
	private static final int MIMILLUS = 29316;
	
	// Location
	private static final Location HELIOS_SPAWN_LOC = new Location(92771, 161909, 3494, 38329);
	private static final Location BLUE_LIGHTNING_SPEAR_LOC = new Location(93208, 161269, 3489);
	private static final Location RED_LIGHTNING_SPEAR_LOC = new Location(92348, 162558, 3489);
	private static final Location MIMILLION_LOC = new Location(92465, 162465, 3487);
	private static final Location MIMILLUS_LOC = new Location(93174, 161394, 3487);
	private static final Location LEOPOLD_LOC = new Location(93531, 162415, 3487);
	private static final Location LEOPOLD_ORIGIN_LOC = new Location(92601, 162196, 3464);
	private static final Location ENUMA_ELISH_ORIGIN_LOC = new Location(92957, 161640, 3485);
	
	// Zone
	private static final int ZONE_ID = 210109;
	
	// Status
	private static final int ALIVE = 0;
	private static final int WAITING = 1;
	private static final int FIGHTING = 2;
	private static final int DEAD = 3;
	
	// Skills
	private static final SkillHolder AUDIENCE_DEBUFF = new SkillHolder(16613, 1);
	private static final SkillHolder RED_LIGHTNING_SPEAR = new SkillHolder(16617, 1);
	private static final SkillHolder BLUE_LIGHTNING_SPEAR = new SkillHolder(16618, 1);
	private static final SkillHolder PRANARACH = new SkillHolder(16624, 1);
	private static final SkillHolder HELIOS_RAGE1 = new SkillHolder(16625, 1);
	private static final SkillHolder HELIOS_RAGE2 = new SkillHolder(16625, 2);
	private static final SkillHolder HELIOS_RAGE3 = new SkillHolder(16625, 3);
	private static final SkillHolder HELIOS_RAGE4 = new SkillHolder(16625, 4);
	private static final SkillHolder LEOPOLD_BOMB = new SkillHolder(16629, 1);
	private static final SkillHolder LEOPOLD_PLASMA_BOMB = new SkillHolder(16630, 1);
	private static final SkillHolder LEOPOLD_ENERGY_BOMB = new SkillHolder(16631, 1);
	private static final SkillHolder LEOPOLD_MINI_GUN = new SkillHolder(16632, 1);
	private static final SkillHolder LEOPOLD_SPRAY_SHOT = new SkillHolder(16633, 1);
	private static final SkillHolder LEOPOLD_HARPOON = new SkillHolder(16634, 1);
	
	// Spawns
	private static final List<SpawnHolder> SPAWNS_MINIONS = new ArrayList<>();
	static
	{
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_TEMPLAR_COLONEL, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_TEMPLAR_COLONEL, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_TEMPLAR_COLONEL, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_TEMPLAR_COLONEL, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_TEMPLAR_COLONEL, HELIOS_SPAWN_LOC, 0, false));
		
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_SHARPSHOOTER, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_SHARPSHOOTER, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_SHARPSHOOTER, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_SHARPSHOOTER, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_SHARPSHOOTER, HELIOS_SPAWN_LOC, 0, false));
		
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_ARCHMAGE, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_ARCHMAGE, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_ARCHMAGE, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_ARCHMAGE, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_ARCHMAGE, HELIOS_SPAWN_LOC, 0, false));
		
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_GATEKEEPER, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_GATEKEEPER, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_GATEKEEPER, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_GATEKEEPER, HELIOS_SPAWN_LOC, 0, false));
		SPAWNS_MINIONS.add(new SpawnHolder(ROYAL_GATEKEEPER, HELIOS_SPAWN_LOC, 0, false));
	}
	
	// Misc
	private static final int HELIOS_RAID_DURATION = 5; // hours
	private static Npc _bossInstance;
	private final NoSummonFriendZone _bossZone;
	private GrandBoss _tempHelios;
	private static List<Npc> _minionSpawns = new ArrayList<>();
	private static Npc _blueLightning;
	private static Npc _redLightning;
	private static Npc _leopold;
	private boolean _activated = false;
	private boolean _stage1 = false;
	private boolean _stage2 = false;
	private boolean _helios80 = false;
	private boolean _helios50 = false;
	private boolean _announce = false;
	protected ScheduledFuture<?> _blueSpearTask;
	protected ScheduledFuture<?> _redSpearTask;
	protected ScheduledFuture<?> _leopoldTask;
	protected ScheduledFuture<?> _debuffTask;
	
	private Helios()
	{
		addAttackId(HELIOS1, HELIOS2, HELIOS3);
		addKillId(HELIOS1, HELIOS2, HELIOS3, MIMILLION, MIMILLUS);
		
		// Zone
		_bossZone = ZoneManager.getInstance().getZoneById(ZONE_ID, NoSummonFriendZone.class);
		
		// Unlock
		final StatSet info = GrandBossManager.getInstance().getStatSet(HELIOS3);
		final int status = GrandBossManager.getInstance().getStatus(HELIOS3);
		if (status == DEAD)
		{
			final long time = info.getLong("respawn_time") - System.currentTimeMillis();
			if (time > 0)
			{
				startQuestTimer("unlock_helios", time, null, null);
			}
			else
			{
				_tempHelios = (GrandBoss) addSpawn(HELIOS3, -126920, -234182, -15563, 0, false, 0);
				GrandBossManager.getInstance().addBoss(_tempHelios);
				GrandBossManager.getInstance().setStatus(HELIOS3, ALIVE);
			}
		}
		else
		{
			_tempHelios = (GrandBoss) addSpawn(HELIOS3, -126920, -234182, -15563, 0, false, 0);
			GrandBossManager.getInstance().addBoss(_tempHelios);
			GrandBossManager.getInstance().setStatus(HELIOS3, ALIVE);
		}
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if ((npc.getId() == HELIOS1) && !_announce)
		{
			_announce = true;
			_debuffTask = ThreadPool.scheduleAtFixedRate(() -> _bossZone.getPlayersInside().forEach(player -> AUDIENCE_DEBUFF.getSkill().applyEffects(player, player)), 5000, 20000);
			Broadcast.toAllOnlinePlayers(new ExShowScreenMessage(NpcStringId.THE_ADEN_WARRIORS_BEGIN_BATTLE_WITH_THE_GIANT_EMPEROR_HELIOS, ExShowScreenMessage.TOP_CENTER, 10000, true));
		}
		
		if ((npc.getId() == HELIOS1) && !_stage1 && (npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)))
		{
			_stage1 = true;
			HELIOS_RAGE1.getSkill().applyEffects(_bossInstance, _bossInstance);
		}
		
		if ((npc.getId() == HELIOS2) && !_activated)
		{
			_activated = true;
			HELIOS_RAGE1.getSkill().applyEffects(_bossInstance, _bossInstance);
			_blueSpearTask = ThreadPool.scheduleAtFixedRate(() ->
			{
				int count = _bossZone.getPlayersInside().size();
				if (count > 0)
				{
					Player randomPlayer = _bossZone.getPlayersInside().get(getRandom(count));
					if (_blueLightning != null)
					{
						_blueLightning.setTarget(randomPlayer);
						_blueLightning.doCast(BLUE_LIGHTNING_SPEAR.getSkill());
					}
				}
				
				_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.HELIOS_PICKS_UP_THE_BLUE_LIGHTNING_SPEAR_AND_BEGINS_GATHERING_HIS_POWER, ExShowScreenMessage.TOP_CENTER, 10000, true));
			}, 10000, 120000);
			_redSpearTask = ThreadPool.scheduleAtFixedRate(() ->
			{
				int count = _bossZone.getPlayersInside().size();
				if (count > 0)
				{
					Player randomPlayer = _bossZone.getPlayersInside().get(getRandom(count));
					if (_redLightning != null)
					{
						_redLightning.setTarget(randomPlayer);
						_redLightning.doCast(RED_LIGHTNING_SPEAR.getSkill());
					}
				}
				
				_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.HELIOS_PICKS_UP_THE_RED_LIGHTNING_SPEAR_AND_BEGINS_GATHERING_HIS_POWER, ExShowScreenMessage.TOP_CENTER, 10000, true));
			}, 30000, 120000);
			_leopoldTask = ThreadPool.scheduleAtFixedRate(() ->
			{
				int count = _bossZone.getPlayersInside().size();
				if (count > 0)
				{
					Player randomPlayer = _bossZone.getPlayersInside().get(getRandom(count));
					if (_leopold != null)
					{
						_leopold.setTarget(randomPlayer);
						int rnd = getRandom(100);
						if (rnd < 16)
						{
							_leopold.doCast(LEOPOLD_BOMB.getSkill());
						}
						else if (rnd < 32)
						{
							_leopold.doCast(LEOPOLD_PLASMA_BOMB.getSkill());
						}
						else if (rnd < 48)
						{
							_leopold.doCast(LEOPOLD_ENERGY_BOMB.getSkill());
						}
						else if (rnd < 64)
						{
							_leopold.doCast(LEOPOLD_MINI_GUN.getSkill());
						}
						else if (rnd < 80)
						{
							_leopold.doCast(LEOPOLD_SPRAY_SHOT.getSkill());
						}
						else
						{
							_leopold.doCast(LEOPOLD_HARPOON.getSkill());
						}
					}
				}
			}, 5000, 10000);
		}
		
		if ((npc.getId() == HELIOS2) && !_stage2 && (npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)))
		{
			_stage2 = true;
			HELIOS_RAGE2.getSkill().applyEffects(_bossInstance, _bossInstance);
		}
		
		if ((npc.getId() == HELIOS3) && !_activated)
		{
			_activated = true;
			HELIOS_RAGE3.getSkill().applyEffects(_bossInstance, _bossInstance);
			_leopoldTask = ThreadPool.scheduleAtFixedRate(() ->
			{
				int count = _bossZone.getPlayersInside().size();
				if (count > 0)
				{
					Player randomPlayer = _bossZone.getPlayersInside().get(getRandom(count));
					if (_leopold != null)
					{
						_leopold.setTarget(randomPlayer);
						int rnd = getRandom(100);
						if (rnd < 16)
						{
							_leopold.doCast(LEOPOLD_BOMB.getSkill());
						}
						else if (rnd < 32)
						{
							_leopold.doCast(LEOPOLD_PLASMA_BOMB.getSkill());
						}
						else if (rnd < 48)
						{
							_leopold.doCast(LEOPOLD_ENERGY_BOMB.getSkill());
						}
						else if (rnd < 64)
						{
							_leopold.doCast(LEOPOLD_MINI_GUN.getSkill());
						}
						else if (rnd < 80)
						{
							_leopold.doCast(LEOPOLD_SPRAY_SHOT.getSkill());
						}
						else
						{
							_leopold.doCast(LEOPOLD_HARPOON.getSkill());
						}
					}
				}
			}, 5000, 10000);
		}
		
		if ((npc.getId() == HELIOS3) && !_helios80 && (npc.getCurrentHp() <= (npc.getMaxHp() * 0.8)))
		{
			_helios80 = true;
			addSpawn(LEOPOLD_ORIGIN, LEOPOLD_ORIGIN_LOC, false, 0);
			addSpawn(ENUMA_ELISH_ORIGIN, ENUMA_ELISH_ORIGIN_LOC, false, 0);
			_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.THE_KAMAEL_ORIGINS_ABOVE_THE_THRONE_OF_HELIOS_BEGIN_TO_SOAR, ExShowScreenMessage.TOP_CENTER, 10000, true));
		}
		else if ((npc.getId() == HELIOS3) && !_helios50 && (npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)))
		{
			_helios50 = true;
			HELIOS_RAGE4.getSkill().applyEffects(_bossInstance, _bossInstance);
			_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.HELIOS_USES_THE_PRANARACH_SHIELD_OF_LIGHT_TO_MINIMIZE_DAMAGE, ExShowScreenMessage.TOP_CENTER, 10000, true));
			_bossInstance.abortCast();
			_bossInstance.doCast(PRANARACH.getSkill());
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "unlock_helios":
			{
				_tempHelios = (GrandBoss) addSpawn(HELIOS3, -126920, -234182, -15563, 0, false, 0);
				GrandBossManager.getInstance().addBoss(_tempHelios);
				GrandBossManager.getInstance().setStatus(HELIOS3, ALIVE);
				break;
			}
			case "beginning":
			{
				if (GrandBossManager.getInstance().getStatus(HELIOS3) == WAITING)
				{
					GrandBossManager.getInstance().setStatus(HELIOS3, FIGHTING);
					_bossInstance = addSpawn(HELIOS1, HELIOS_SPAWN_LOC.getX(), HELIOS_SPAWN_LOC.getY(), HELIOS_SPAWN_LOC.getZ(), HELIOS_SPAWN_LOC.getHeading(), false, 0, false);
					for (SpawnHolder spawn : SPAWNS_MINIONS)
					{
						_minionSpawns.add(addSpawn(spawn.getNpcId(), spawn.getLocation()));
					}
					
					startQuestTimer("resetRaid", HELIOS_RAID_DURATION * 60 * 60 * 1000, _bossInstance, null);
				}
				break;
			}
			case "resetRaid":
			{
				final int status = GrandBossManager.getInstance().getStatus(HELIOS3);
				if ((status > ALIVE) && (status < DEAD))
				{
					_bossZone.oustAllPlayers();
					Broadcast.toAllOnlinePlayers(new ExShowScreenMessage(NpcStringId.THE_HEROES_DRAINED_OF_THEIR_POWERS_HAVE_BEEN_BANISHED_FROM_THE_THRONE_OF_HELIOS_BY_HELIOS_POWERS, ExShowScreenMessage.TOP_CENTER, 10000, true));
					GrandBossManager.getInstance().setStatus(HELIOS3, ALIVE);
					clean();
				}
				break;
			}
			case "stage2":
			{
				_bossInstance = addSpawn(HELIOS2, HELIOS_SPAWN_LOC.getX(), HELIOS_SPAWN_LOC.getY(), HELIOS_SPAWN_LOC.getZ(), HELIOS_SPAWN_LOC.getHeading(), false, 0, false);
				_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.HELIOS_APPEARANCE_CHANGES_AND_HE_BEGINS_TO_GROW_STRONGER, ExShowScreenMessage.TOP_CENTER, 10000, true));
				for (SpawnHolder spawn : SPAWNS_MINIONS)
				{
					_minionSpawns.add(addSpawn(spawn.getNpcId(), spawn.getLocation()));
				}
				
				startQuestTimer("spheresSpawn", 10000, null, null);
				break;
			}
			case "stage3":
			{
				_activated = false;
				_bossInstance = addSpawn(HELIOS3, HELIOS_SPAWN_LOC.getX(), HELIOS_SPAWN_LOC.getY(), HELIOS_SPAWN_LOC.getZ(), HELIOS_SPAWN_LOC.getHeading(), false, 0, false);
				_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.HELIOS_APPEARANCE_CHANGES_AND_HE_BEGINS_TO_GROW_STRONGER, ExShowScreenMessage.TOP_CENTER, 10000, true));
				startQuestTimer("leopoldSpawn", 10000, null, null);
				break;
			}
			case "spheresSpawn":
			{
				_blueLightning = addSpawn(HELIOS_BLUE_LIGHTNING, BLUE_LIGHTNING_SPEAR_LOC, false, 0);
				_redLightning = addSpawn(HELIOS_RED_LIGHTNING, RED_LIGHTNING_SPEAR_LOC, false, 0);
				_blueLightning.setInvul(true);
				_redLightning.setInvul(true);
				_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.THE_ENUMA_ELISH_SPEAR_ON_THE_THRONE_OF_HELIOS_IS_PREPARED_AND_PLACED_IN_POSITION, ExShowScreenMessage.TOP_CENTER, 10000, true));
				startQuestTimer("protectorsSpawn", 10000, null, null);
				break;
			}
			case "protectorsSpawn":
			{
				_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.MIMILLION_AND_MIMILLUS_APPEAR_IN_ORDER_TO_PROTECT_THE_ENUMA_ELISH_OF_RED_LIGHTNING_AND_THE_ENUMA_ELISH_OF_BLUE_LIGHTNING, ExShowScreenMessage.TOP_CENTER, 10000, true));
				addSpawn(MIMILLION, MIMILLION_LOC, false, 0);
				addSpawn(MIMILLUS, MIMILLUS_LOC, false, 0);
				startQuestTimer("leopoldSpawn", 10000, null, null);
				break;
			}
			case "leopoldSpawn":
			{
				_leopold = addSpawn(LEOPOLD, LEOPOLD_LOC, false, 0);
				_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.THE_SIEGE_CANNON_LEOPOLD_ON_THE_THRONE_OF_HELIOS_BEGINS_TO_PREPARE_TO_FIRE, ExShowScreenMessage.TOP_CENTER, 10000, true));
				break;
			}
		}
		
		return htmltext;
	}
	
	private void clean()
	{
		_bossZone.getCharactersInside().forEach(mob ->
		{
			if (mob.isNpc())
			{
				mob.deleteMe();
			}
		});
		if (_blueSpearTask != null)
		{
			_blueSpearTask.cancel(true);
			_blueSpearTask = null;
		}
		
		if (_redSpearTask != null)
		{
			_redSpearTask.cancel(true);
			_redSpearTask = null;
		}
		
		if (_leopoldTask != null)
		{
			_leopoldTask.cancel(true);
			_leopoldTask = null;
		}
		
		if (_debuffTask != null)
		{
			_debuffTask.cancel(true);
			_debuffTask = null;
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		switch (npc.getId())
		{
			case MIMILLION:
			{
				if (_redLightning != null)
				{
					_redLightning.deleteMe();
					_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.MIMILLION_FALLS_AND_THE_RED_LIGHTNING_SPEAR_VANISHES, ExShowScreenMessage.TOP_CENTER, 10000, true));
				}
				break;
			}
			case MIMILLUS:
			{
				if (_blueLightning != null)
				{
					_blueLightning.deleteMe();
					_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.MIMILLUS_FALLS_AND_THE_BLUE_LIGHTNING_SPEAR_VANISHES, ExShowScreenMessage.TOP_CENTER, 10000, true));
				}
				break;
			}
			case HELIOS1:
			{
				_bossInstance.deleteMe();
				_bossZone.getPlayersInside().forEach(player -> playMovie(player, Movie.SC_HELIOS_TRANS_A));
				startQuestTimer("stage2", 15000, null, null);
				break;
			}
			case HELIOS2:
			{
				_bossInstance.deleteMe();
				if (_leopold != null)
				{
					_leopold.deleteMe();
				}
				
				_bossZone.getPlayersInside().forEach(player -> playMovie(player, Movie.SC_HELIOS_TRANS_B));
				startQuestTimer("stage3", 15000, null, null);
				break;
			}
			case HELIOS3:
			{
				clean();
				_bossZone.broadcastPacket(new ExShowScreenMessage(NpcStringId.HELIOS_DEFEATED_TAKES_FLIGHT_DEEP_IN_TO_THE_SUPERION_FORT_HIS_THRONE_IS_RENDERED_INACTIVE, ExShowScreenMessage.TOP_CENTER, 10000, true));
				
				GrandBossManager.getInstance().setStatus(HELIOS3, DEAD);
				
				final long baseIntervalMillis = GrandBossConfig.HELIOS_SPAWN_INTERVAL * 3600000;
				final long randomRangeMillis = GrandBossConfig.HELIOS_SPAWN_RANDOM * 3600000;
				final long respawnTime = baseIntervalMillis + getRandom(-randomRangeMillis, randomRangeMillis);
				final StatSet info = GrandBossManager.getInstance().getStatSet(HELIOS3);
				info.set("respawn_time", System.currentTimeMillis() + respawnTime);
				GrandBossManager.getInstance().setStatSet(HELIOS3, info);
				startQuestTimer("unlock_helios", respawnTime, null, null);
				break;
			}
		}
	}
	
	public static void main(String[] args)
	{
		new Helios();
	}
}
