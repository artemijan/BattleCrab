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
package ai.bosses.Kelbim;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.config.GrandBossConfig;
import org.l2jmobius.gameserver.data.xml.MapRegionData;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.managers.GrandBossManager;
import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.player.TeleportWhereType;
import org.l2jmobius.gameserver.model.actor.instance.GrandBoss;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.QuestTimer;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.Earthquake;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;
import org.l2jmobius.gameserver.util.Broadcast;

/**
 * Kelbim AI
 * @author LasTravel
 * @video https://www.youtube.com/watch?v=qVkk2BJoGoU
 */
public class Kelbim extends Script
{
	// Status
	private static final int ALIVE = 0;
	private static final int WAITING = 1;
	private static final int FIGHTING = 2;
	private static final int DEAD = 3;
	
	// NPCs
	private static final int ENTER_DEVICE = 34052;
	private static final int TELEPORT_DEVICE = 34053;
	private static final int KELBIM_SHOUT = 19597;
	private static final int KELBIM = 26124;
	private static final int GUARDIAN_SINISTRA = 26126;
	private static final int GUARDIAN_DESTRA = 26127;
	private static final int[] KELBIM_GUARDIANS =
	{
		GUARDIAN_SINISTRA,
		GUARDIAN_DESTRA
	};
	private static final int KELBIM_GUARD = 26129;
	private static final int KELBIM_ALTAR = 26130;
	private static final int[] KELBIM_MINIONS =
	{
		GUARDIAN_SINISTRA,
		GUARDIAN_DESTRA,
		KELBIM_GUARD
	};
	private static final int[] ALL_MONSTERS =
	{
		KELBIM,
		KELBIM_MINIONS[0],
		KELBIM_MINIONS[1],
		KELBIM_MINIONS[2],
		KELBIM_ALTAR
	};
	
	// Doors
	private static final int DOOR1 = 18190002;
	private static final int DOOR2 = 18190004;
	
	// Skills
	private static final Skill METEOR_CRASH = SkillData.getInstance().getSkill(23692, 1);
	private static final Skill WATER_DROP = SkillData.getInstance().getSkill(23693, 1);
	private static final Skill TORNADO_SACKLE = SkillData.getInstance().getSkill(23694, 1);
	private static final Skill FLAME_THROWER = SkillData.getInstance().getSkill(23699, 1);
	private static final Skill[] AREA_SKILLS =
	{
		METEOR_CRASH,
		WATER_DROP,
		TORNADO_SACKLE,
		FLAME_THROWER
	};
	
	// Misc
	private static final ZoneType ZONE = ZoneManager.getInstance().getZoneById(60023);
	private static final Location KELBIM_LOCATION = new Location(-55386, 58939, -274);
	
	// Vars
	private static GrandBoss _kelbimBoss;
	private static long _lastAction;
	private static int _bossStage;
	private static List<Npc> _minions = new ArrayList<>();
	
	public Kelbim()
	{
		addTalkId(ENTER_DEVICE, TELEPORT_DEVICE);
		addStartNpc(ENTER_DEVICE, TELEPORT_DEVICE);
		addFirstTalkId(ENTER_DEVICE, TELEPORT_DEVICE);
		addAttackId(ALL_MONSTERS);
		addKillId(KELBIM);
		
		// Unlock
		final StatSet info = GrandBossManager.getInstance().getStatSet(KELBIM);
		final int status = GrandBossManager.getInstance().getStatus(KELBIM);
		if (status == DEAD)
		{
			final long time = info.getLong("respawn_time") - System.currentTimeMillis();
			if (time > 0)
			{
				startQuestTimer("unlock_kelbim", time, null, null);
			}
			else
			{
				openDoor(DOOR1, 0);
				openDoor(DOOR2, 0);
				GrandBossManager.getInstance().setStatus(KELBIM, ALIVE);
			}
		}
		else if (status != ALIVE)
		{
			openDoor(DOOR1, 0);
			openDoor(DOOR2, 0);
			GrandBossManager.getInstance().setStatus(KELBIM, ALIVE);
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "unlock_kelbim":
			{
				GrandBossManager.getInstance().setStatus(KELBIM, ALIVE);
				Broadcast.toAllOnlinePlayers(new Earthquake(-55754, 59903, -269, 20, 10));
				openDoor(DOOR1, 0);
				openDoor(DOOR2, 0);
				break;
			}
			case "check_activity_task":
			{
				if ((_lastAction + 900000) < System.currentTimeMillis())
				{
					GrandBossManager.getInstance().setStatus(KELBIM, ALIVE);
					for (Creature creature : ZONE.getCharactersInside())
					{
						if (creature != null)
						{
							if (creature.isNpc())
							{
								creature.deleteMe();
							}
							else if (creature.isPlayer())
							{
								creature.teleToLocation(MapRegionData.getInstance().getTeleToLocation(creature, TeleportWhereType.TOWN));
							}
						}
					}
					
					startQuestTimer("end_kelbim", 2000, null, null);
				}
				else
				{
					startQuestTimer("check_activity_task", 60000, null, null);
				}
				break;
			}
			case "stage_1_start":
			{
				_bossStage = 1;
				GrandBossManager.getInstance().setStatus(KELBIM, FIGHTING);
				playMovie(ZONE.getPlayersInside(), Movie.SC_KELBIM_OPENING);
				startQuestTimer("stage_1_kelbim_spawn", 25000, null, null);
				break;
			}
			case "stage_1_kelbim_spawn":
			{
				_kelbimBoss = (GrandBoss) addSpawn(KELBIM, -56340, 60801, -269, 54262, false, 0);
				GrandBossManager.getInstance().addBoss(_kelbimBoss);
				_lastAction = System.currentTimeMillis();
				startQuestTimer("check_activity_task", 60000, null, null);
				startQuestTimer("stage_all_random_area_attack", getRandom(2, 3) * 60000, null, null);
				break;
			}
			case "stage_all_spawn_minions":
			{
				for (int i = 0; i < getRandom((_bossStage * 5) / 2, _bossStage * 5); i++)
				{
					final Npc minion = addSpawn(KELBIM_GUARD, _kelbimBoss.getX(), _kelbimBoss.getY(), _kelbimBoss.getZ(), 0, true, 0, true, 0);
					minion.setRunning();
					minion.asAttackable().setIsRaidMinion(true);
					_minions.add(minion);
				}
				
				for (int i = 0; i < getRandom((_bossStage * 2) / 2, _bossStage * 2); i++)
				{
					final Npc minion = addSpawn(KELBIM_GUARDIANS[getRandom(KELBIM_GUARDIANS.length)], _kelbimBoss.getX(), _kelbimBoss.getY(), _kelbimBoss.getZ(), 0, true, 0, true, 0);
					minion.setRunning();
					minion.asAttackable().setIsRaidMinion(true);
					_minions.add(minion);
				}
				break;
			}
			case "stage_all_random_area_attack":
			{
				if ((_bossStage > 0) && (_bossStage < 7))
				{
					if (_kelbimBoss.isInCombat())
					{
						final Skill randomAttackSkill = AREA_SKILLS[getRandom(AREA_SKILLS.length)];
						final List<Npc> skillNpcs = new ArrayList<>();
						for (Player pl : ZONE.getPlayersInside())
						{
							if (pl == null)
							{
								continue;
							}
							
							if (getRandom(100) > 40)
							{
								final Npc skillMob = addSpawn(KELBIM_SHOUT, pl.getX(), pl.getY(), pl.getZ() + 20, 0, true, 60000, false, 0);
								skillNpcs.add(skillMob);
								_minions.add(skillMob);
							}
						}
						
						for (Npc skillNpc : skillNpcs)
						{
							if (skillNpc == null)
							{
								continue;
							}
							
							skillNpc.doCast(randomAttackSkill);
						}
					}
					
					startQuestTimer("stage_all_random_area_attack", getRandom(1, 2) * 60000, null, null);
				}
				break;
			}
			case "cancel_timers":
			{
				final QuestTimer activityTimer = getQuestTimer("check_activity_task", null, null);
				if (activityTimer != null)
				{
					activityTimer.cancel();
				}
				break;
			}
			case "end_kelbim":
			{
				_bossStage = 0;
				ZONE.oustAllPlayers();
				if (_kelbimBoss != null)
				{
					_kelbimBoss.deleteMe();
				}
				
				if (!_minions.isEmpty())
				{
					for (Npc minion : _minions)
					{
						if (minion == null)
						{
							continue;
						}
						
						minion.deleteMe();
					}
				}
				
				_minions.clear();
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		switch (npc.getId())
		{
			case TELEPORT_DEVICE:
			{
				player.teleToLocation(-55730, 55643, -1954);
				return null;
			}
			case ENTER_DEVICE:
			{
				return "34052.html";
			}
		}
		
		return super.onFirstTalk(npc, player);
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		if (npc.getId() == ENTER_DEVICE)
		{
			final int status = GrandBossManager.getInstance().getStatus(KELBIM);
			if (status > ALIVE)
			{
				return "34052-1.html";
			}
			
			if (!player.isInParty())
			{
				final NpcHtmlMessage packet = new NpcHtmlMessage(npc.getObjectId());
				packet.setHtml(getHtm(player, "34052-2.html"));
				packet.replace("%min%", Integer.toString(GrandBossConfig.KELBIM_MIN_PLAYERS));
				player.sendPacket(packet);
				return null;
			}
			
			final Party party = player.getParty();
			final boolean isInCC = party.isInCommandChannel();
			final List<Player> members = (isInCC) ? party.getCommandChannel().getMembers() : party.getMembers();
			final boolean isPartyLeader = (isInCC) ? party.getCommandChannel().isLeader(player) : party.isLeader(player);
			for (Player member : members)
			{
				if (!member.isInsideRadius3D(npc, 1000))
				{
					return "34052-2.html";
				}
			}
			
			if (!isPartyLeader)
			{
				return "34052-3.html";
			}
			else if ((members.size() < GrandBossConfig.KELBIM_MIN_PLAYERS) || (members.size() > GrandBossConfig.KELBIM_MAX_PLAYERS))
			{
				final NpcHtmlMessage packet = new NpcHtmlMessage(npc.getObjectId());
				packet.setHtml(getHtm(player, "34052-2.html"));
				packet.replace("%min%", Integer.toString(GrandBossConfig.KELBIM_MIN_PLAYERS));
				player.sendPacket(packet);
			}
			else
			{
				for (Player member : members)
				{
					if ((status == ALIVE) && member.isInsideRadius3D(npc, 1000))
					{
						GrandBossManager.getInstance().setStatus(KELBIM, WAITING);
						startQuestTimer("stage_1_start", GrandBossConfig.KELBIM_WAIT_TIME * 60 * 1000, null, null);
						member.teleToLocation(KELBIM_LOCATION, true);
					}
					else
					{
						return "34052-3.html";
					}
				}
			}
		}
		
		return super.onTalk(npc, player);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isPet)
	{
		if (npc.getId() == KELBIM)
		{
			_lastAction = System.currentTimeMillis();
			
			switch (_bossStage)
			{
				case 1:
				{
					if (npc.getCurrentHp() < (npc.getMaxHp() * 0.80))
					{
						_bossStage = 2;
						notifyEvent("stage_all_spawn_minions", null, null);
					}
					break;
				}
				case 2:
				{
					if (npc.getCurrentHp() < (npc.getMaxHp() * 0.60))
					{
						_bossStage = 3;
						notifyEvent("stage_all_spawn_minions", null, null);
					}
					break;
				}
				case 3:
				{
					if (npc.getCurrentHp() < (npc.getMaxHp() * 0.40))
					{
						_bossStage = 4;
						notifyEvent("stage_all_spawn_minions", null, null);
					}
					break;
				}
				case 4:
				{
					if (npc.getCurrentHp() < (npc.getMaxHp() * 0.20))
					{
						_bossStage = 5;
						notifyEvent("stage_all_spawn_minions", null, null);
					}
					break;
				}
				case 5:
				{
					if (npc.getCurrentHp() < (npc.getMaxHp() * 0.05))
					{
						_bossStage = 6;
						notifyEvent("stage_all_spawn_minions", null, null);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isPet)
	{
		_bossStage = 7;
		addSpawn(TELEPORT_DEVICE, -54331, 58331, -264, 16292, false, 1800000);
		notifyEvent("cancel_timers", null, null);
		closeDoor(DOOR1, 0);
		closeDoor(DOOR2, 0);
		
		GrandBossManager.getInstance().setStatus(KELBIM, DEAD);
		
		final long baseIntervalMillis = GrandBossConfig.KELBIM_SPAWN_INTERVAL * 3600000;
		final long randomRangeMillis = GrandBossConfig.KELBIM_SPAWN_RANDOM * 3600000;
		final long respawnTime = baseIntervalMillis + getRandom(-randomRangeMillis, randomRangeMillis);
		final StatSet info = GrandBossManager.getInstance().getStatSet(KELBIM);
		info.set("respawn_time", System.currentTimeMillis() + respawnTime);
		GrandBossManager.getInstance().setStatSet(KELBIM, info);
		startQuestTimer("unlock_kelbim", respawnTime, null, null);
		startQuestTimer("end_kelbim", 1800000, null, null);
	}
	
	public static void main(String[] args)
	{
		new Kelbim();
	}
}
