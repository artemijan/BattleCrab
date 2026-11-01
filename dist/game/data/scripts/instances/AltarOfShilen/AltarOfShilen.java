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
package instances.AltarOfShilen;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Future;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.managers.InstanceManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.Earthquake;
import org.l2jmobius.gameserver.network.serverpackets.ExSendUIEvent;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * Instance Altar of Shilen
 * @URL https://l2wiki.com/Altar_of_Shilen
 * @author Gigi
 * @date 2018-04-13 - [23:27:28]
 */
public class AltarOfShilen extends InstanceScript
{
	// NPCs
	private static final int LOGART_VAN_DYKE = 34331;
	private static final int DINFORD = 34332;
	private static final int ISADORA = 25856;
	private static final int MELISSA0 = 25855;
	private static final int MELISSA = 25876; // She dropping items
	private static final int EXECUTOR_CAPTAIN = 23131;
	private static final int ETINA_BLADESMAN = 23138;
	private static final int CORRUPTED_CAPTAIN = 25857;
	private static final int CORRUPTED_HIGH_PRIEST = 25858;
	private static final int RITUAL_ALTAR0 = 19121;
	private static final int RITUAL_ALTAR1 = 19122;
	private static final int SHILLIEN_ALTAR = 19123;
	private static final int INVISIBLE = 8561;
	private static final int ETINA_GOSPEL = 23132;
	private static final int ETINA_PRIEST = 23133;
	private static final int ETINA_PROTECTORS = 23134;
	private static final int ETINA_PUNISHERS = 23135;
	private static final int ETINA_COMMINATION = 23136;
	private static final int ETINA_DARKMONAGERS = 23137;
	private static final int ELITE_ESCORT = 23126;
	private static final int ELITE_CHASER = 23127;
	private static final int ELITE_WARRIOR = 23128;
	private static final int ELITE_ALCHEMIST = 23129;
	private static final int ELITE_PRIEST = 23130;
	private static final int EMBRYO_RESERCHER = 23139;
	private static final int EMBRYO_WATCHMAN = 23140;
	private static final int EMBRYO_FIGHTER = 23141;
	private static final int EMBRYO_GUARD = 23142;
	
	// Skills
	static final SkillHolder PROTECTED_ALTAR = new SkillHolder(14496, 1);
	
	// Misc
	private static final int TEMPLATE_ID = 194;
	private static final int ALTAR_TIME = 180;
	// @formatter:off
	private static final int[] DOORS = { 25180001, 25180002, 25180003, 25180004, 25180005, 25180006, 25180007 };
	// @formatter:on
	private static final NpcStringId[] SHOUT_MSG =
	{
		NpcStringId.I_NEED_HELP,
		NpcStringId.FOCUS_FIRE_ACCORDING_TO_MY_ORCHESTRATION,
		NpcStringId.I_NEED_HEAL,
		NpcStringId.I_M_GOING_TO_BACK_OFF_FOR_A_BIT,
		NpcStringId.I_WILL_ATTACK_THE_ENEMY_S_HEALER,
		NpcStringId.STOP_ENEMIES_HEALING,
		NpcStringId.ATTACK_WEAKENED_ENEMY,
		NpcStringId.COME_AT_ME,
		NpcStringId.SWITCH_TO_FAST_SHOOTING_MODE,
		NpcStringId.SWITCH_TO_FAST_CASTING_MODE,
		NpcStringId.BLINK_ATTACK,
		NpcStringId.ONLY_DEATH_AWAITS_FOR_THE_WEAK,
	};
	
	// Locations
	static final Location MELISSA_SPAWN = new Location(178146, 14356, -13688);
	private static final Location SECOND_FLOOR = new Location(179357, 13664, -9828);
	private static final Location THIRD_FLOOR = new Location(179354, 12922, -12776);
	
	// Misc
	final List<Player> _playersInside = new ArrayList<>();
	final Map<Integer, Integer> _killedMonsters = new ConcurrentHashMap<>();
	static List<Npc> _firstFloorVictims = new ArrayList<>();
	public static Future<?> _timer;
	public Npc _altar;
	
	public AltarOfShilen()
	{
		super(TEMPLATE_ID);
		addInstanceCreatedId(TEMPLATE_ID);
		addFirstTalkId(LOGART_VAN_DYKE, RITUAL_ALTAR0, RITUAL_ALTAR1, DINFORD);
		addSpawnId(CORRUPTED_CAPTAIN, CORRUPTED_HIGH_PRIEST, INVISIBLE, SHILLIEN_ALTAR);
		addAttackId(ETINA_GOSPEL, ETINA_PRIEST, ETINA_PROTECTORS, ETINA_PUNISHERS, ETINA_COMMINATION, ETINA_DARKMONAGERS, ELITE_ESCORT, ELITE_CHASER, ELITE_WARRIOR, ELITE_ALCHEMIST, ELITE_PRIEST, EMBRYO_RESERCHER, EMBRYO_WATCHMAN, EMBRYO_FIGHTER, EMBRYO_GUARD);
		addKillId(EXECUTOR_CAPTAIN, CORRUPTED_CAPTAIN, ETINA_BLADESMAN, CORRUPTED_HIGH_PRIEST, ETINA_GOSPEL, ETINA_PROTECTORS, ETINA_PUNISHERS, MELISSA0, MELISSA, ISADORA);
	}
	
	@Override
	public void onInstanceCreated(Instance instance, Player player)
	{
		instance.spawnGroup("general");
		instance.spawnGroup("first_floor");
		instance.setStatus(1);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final Instance world = npc.getInstanceWorld();
		if (event.equals("enterInstance"))
		{
			enterInstance(player, npc, TEMPLATE_ID);
			if (player.isGM())
			{
				_playersInside.add(player);
			}
			
			final Party party = player.getParty();
			if (party != null)
			{
				for (Player member : party.getMembers())
				{
					_playersInside.add(member);
				}
			}
		}
		
		if (event.equals("check_player"))
		{
			World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 400, p ->
			{
				if ((p != null) && p.isPlayer() && !p.isDead())
				{
					npc.setScriptValue(1);
					final Npc gospel = addSpawn(ETINA_GOSPEL, npc, true, 0, false, world.getId());
					final Npc priest = addSpawn(ETINA_PRIEST, npc, true, 0, false, world.getId());
					final Npc protectors = addSpawn(ETINA_PROTECTORS, npc, true, 0, false, world.getId());
					final Npc punishers = addSpawn(ETINA_PUNISHERS, npc, true, 0, false, world.getId());
					final Npc commination = addSpawn(ETINA_COMMINATION, npc, true, 0, false, world.getId());
					final Npc darkmonagers = addSpawn(ETINA_DARKMONAGERS, npc, true, 0, false, world.getId());
					addAttackPlayerDesire(gospel, p);
					addAttackPlayerDesire(priest, p);
					addAttackPlayerDesire(protectors, p);
					addAttackPlayerDesire(punishers, p);
					addAttackPlayerDesire(commination, p);
					addAttackPlayerDesire(darkmonagers, p);
				}
			});
		}
		
		if (isInInstance(world))
		{
			switch (npc.getId())
			{
				case LOGART_VAN_DYKE:
				{
					if (event.equals("open_door"))
					{
						if (world.getStatus() == 1)
						{
							if (world.getDoor(DOORS[0]) != null)
							{
								world.getDoor(DOORS[0]).openMe();
							}
							
							world.setStatus(2);
							onStatusChanged(world);
							break;
						}
						else if (world.getStatus() == 3)
						{
							if (world.getDoor(DOORS[1]) != null)
							{
								world.getDoor(DOORS[1]).openMe();
							}
							
							world.setStatus(4);
							onStatusChanged(world);
							break;
						}
						else if (world.getStatus() == 5)
						{
							if (world.getDoor(DOORS[2]) != null)
							{
								world.getDoor(DOORS[2]).openMe();
							}
							
							world.setStatus(6);
							onStatusChanged(world);
							break;
						}
					}
					break;
				}
				case RITUAL_ALTAR0:
				{
					if ((world.getStatus() == 3) && event.equals("teleport1"))
					{
						World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1000, p ->
						{
							if ((p != null) && p.isPlayable() && !p.isDead())
							{
								p.teleToLocation(SECOND_FLOOR, world.getTemplateId());
							}
						});
						if (!world.getParameters().getBoolean("second_floor_spawned", false))
						{
							world.setParameter("second_floor_spawned", true);
							world.spawnGroup("second_floor");
						}
					}
					break;
				}
				case RITUAL_ALTAR1:
				{
					if ((world.getStatus() == 5) && event.equals("teleport2"))
					{
						World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1000, p ->
						{
							if ((p != null) && p.isPlayable() && !p.isDead())
							{
								p.teleToLocation(THIRD_FLOOR, world.getTemplateId());
							}
						});
						if (!world.getParameters().getBoolean("third_floor_spawned", false))
						{
							world.setParameter("third_floor_spawned", true);
							world.spawnGroup("third_floor");
						}
					}
					break;
				}
				case DINFORD:
				{
					if ((world.getStatus() == 6) && event.equals("final_door"))
					{
						if ((world.getDoor(DOORS[6]) != null) || !world.getDoor(DOORS[6]).isOpen())
						{
							world.getDoor(DOORS[6]).openMe();
						}
						
						world.setStatus(7);
						onStatusChanged(world);
						cancelQuestTimer("check_player", npc, null);
						npc.deleteMe();
					}
					break;
				}
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if ((npc.getInstanceWorld() != null) && (getRandom(30) < 3))
		{
			npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), getRandomEntry(SHOUT_MSG)));
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			final int npcId = npc.getId();
			if (!_killedMonsters.containsKey(npcId))
			{
				_killedMonsters.put(npcId, 1);
			}
			else
			{
				_killedMonsters.put(npcId, _killedMonsters.get(npcId) + 1);
			}
			
			// 1st Floor
			if (world.getStatus() == 2)
			{
				if (_killedMonsters.containsKey(EXECUTOR_CAPTAIN) && (_killedMonsters.get(EXECUTOR_CAPTAIN) >= 3))
				{
					final int boos1 = world.getTemplateParameters().getInt("boss1");
					final Npc captain = world.getNpc(boos1);
					if (captain != null)
					{
						captain.setInvul(false);
					}
				}
				
				if (_killedMonsters.containsKey(CORRUPTED_CAPTAIN) && (_killedMonsters.get(CORRUPTED_CAPTAIN) >= 1))
				{
					for (Player player : _playersInside)
					{
						player.sendPacket(new ExShowScreenMessage(NpcStringId.YOU_CAN_MOVE_TO_THE_NEXT_FLOOR_THROUGH_THE_ALTAR_OF_SACRIFICE, ExShowScreenMessage.MIDDLE_CENTER, 5000));
					}
					
					world.setStatus(3);
					_killedMonsters.clear();
					onStatusChanged(world);
				}
			}
			// 2st Floor
			else if (world.getStatus() == 4)
			{
				if (_killedMonsters.containsKey(ETINA_BLADESMAN) && (_killedMonsters.get(ETINA_BLADESMAN) >= 3))
				{
					final int boos2 = world.getTemplateParameters().getInt("boss2");
					final Npc priest = world.getNpc(boos2);
					if (priest != null)
					{
						priest.setInvul(false);
					}
				}
				
				if (_killedMonsters.containsKey(CORRUPTED_HIGH_PRIEST) && (_killedMonsters.get(CORRUPTED_HIGH_PRIEST) >= 1))
				{
					for (Player player : _playersInside)
					{
						player.sendPacket(new ExShowScreenMessage(NpcStringId.YOU_CAN_MOVE_TO_THE_NEXT_FLOOR_THROUGH_THE_ALTAR_OF_SACRIFICE, ExShowScreenMessage.MIDDLE_CENTER, 5000));
					}
					
					world.setStatus(5);
					_killedMonsters.clear();
					onStatusChanged(world);
					
				}
			}
			// 3st Floor
			else if (world.getStatus() == 6)
			{
				switch (npc.getId())
				{
					case ETINA_GOSPEL:
					{
						if (world.getDoor(DOORS[3]) != null)
						{
							world.getDoor(DOORS[3]).openMe();
							ThreadPool.schedule(() -> world.getDoor(DOORS[3]).closeMe(), 5000);
						}
						break;
					}
					case ETINA_PROTECTORS:
					{
						if (world.getDoor(DOORS[4]) != null)
						{
							world.getDoor(DOORS[4]).openMe();
							ThreadPool.schedule(() -> world.getDoor(DOORS[4]).closeMe(), 5000);
						}
						break;
					}
					case ETINA_PUNISHERS:
					{
						if (world.getDoor(DOORS[5]) != null)
						{
							world.getDoor(DOORS[5]).openMe();
							ThreadPool.schedule(() -> world.getDoor(DOORS[5]).closeMe(), 5000);
						}
						break;
					}
				}
			}
			else if (world.getStatus() == 8)
			{
				if (_killedMonsters.containsKey(MELISSA) && (_killedMonsters.get(MELISSA) == 1))
				{
					for (Player player : _playersInside)
					{
						player.sendPacket(new ExShowScreenMessage(NpcStringId.ALTAR_OF_SHILEN_HAS_BEEN_DESTROYED_YOU_VE_WON, ExShowScreenMessage.TOP_CENTER, 5000, true));
					}
					
					_playersInside.clear();
					_killedMonsters.clear();
					world.finishInstance(1);
				}
			}
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		if (npc.getInstanceWorld() != null)
		{
			switch (npc.getId())
			{
				case CORRUPTED_CAPTAIN:
				case CORRUPTED_HIGH_PRIEST:
				{
					npc.setInvul(true);
					break;
				}
				case INVISIBLE:
				{
					if (npc.isScriptValue(0))
					{
						startQuestTimer("check_player", 2000, npc, null, true);
					}
					break;
				}
				case SHILLIEN_ALTAR:
				{
					_altar = npc;
					break;
				}
			}
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final Instance world = npc.getInstanceWorld();
		final String htmltext = null;
		if (isInInstance(world))
		{
			switch (npc.getId())
			{
				case LOGART_VAN_DYKE:
				{
					if ((world.getStatus() > 0) && (world.getStatus() < 3))
					{
						return "34331.html";
					}
					else if ((world.getStatus() >= 3) && (world.getStatus() < 5))
					{
						return "34331-01.html";
					}
					else if (world.getStatus() >= 5)
					{
						return "34331-02.html";
					}
					break;
				}
				case RITUAL_ALTAR0:
				{
					if (world.getStatus() >= 3)
					{
						return "19121.html";
					}
					break;
				}
				case RITUAL_ALTAR1:
				{
					if (world.getStatus() >= 5)
					{
						return "19122.html";
					}
					break;
				}
				case DINFORD:
				{
					if (world.getStatus() >= 5)
					{
						return "34332.html";
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	public static class VictimDefeatTask implements Runnable
	{
		private final Instance _world;
		private int _victims;
		private final int _initialTime;
		private int _time;
		
		VictimDefeatTask(int victims, int time, Instance world)
		{
			_world = world;
			_victims = victims;
			_initialTime = time;
			_time = time;
		}
		
		@Override
		public void run()
		{
			for (Player player : _world.getPlayers())
			{
				player.sendPacket(new ExSendUIEvent(player, ExSendUIEvent.TYPE_GP_TIMER, (_time--), 0, NpcStringId.SACRIFICE_LEFT_S1, String.valueOf(_victims)));
			}
			
			if (_time <= 0)
			{
				_time = _initialTime;
				--_victims;
				for (Player player : _world.getPlayers())
				{
					player.sendPacket(new Earthquake(player.getX(), player.getY(), player.getZ(), 20, 5));
					player.sendPacket(new ExShowScreenMessage(NpcStringId.SACRIFICE_HAS_BEEN_KILLED_SACRIFICE_LEFT_S1, ExShowScreenMessage.MIDDLE_CENTER, 3000, String.valueOf(_victims)));
				}
			}
			
			if ((_world.getStatus() == 2) && !_firstFloorVictims.isEmpty())
			{
				_firstFloorVictims.get(0).deleteMe();
				_firstFloorVictims.remove(0);
			}
			
			if ((_victims == 1) && (_world.getStatus() == 2))
			{
				for (Player player : _world.getPlayers())
				{
					player.sendPacket(new ExShowScreenMessage(NpcStringId.FOUR_LIVES_WERE_SACRIFICED_AND_THE_BLESSING_OF_BLOOD_IS_BESTOWED_UPON_THE_CORRUPTED_CAPTAIN, ExShowScreenMessage.MIDDLE_CENTER, 10000));
				}
			}
			
			if ((_victims == 1) && (_world.getStatus() == 4))
			{
				for (Player player : _world.getPlayers())
				{
					player.sendPacket(new ExShowScreenMessage(NpcStringId.FOUR_LIVES_WERE_SACRIFICED_AND_THE_BLESSING_OF_BLOOD_IS_BESTOWED_UPON_THE_CORRUPTED_HIGH_PRIEST, ExShowScreenMessage.MIDDLE_CENTER, 10000));
				}
			}
			
			if (_victims <= 0)
			{
				ThreadPool.schedule(() ->
				{
					for (Player player : _world.getPlayers())
					{
						player.sendPacket(new ExShowScreenMessage(NpcStringId.ALL_OFFERINGS_WERE_SACRIFICED_AND_THE_BLESSING_OF_BLOOD_IS_BESTOWED_UPON_THE_EMBRYO_COLONY_IN_THAT_FLOOR, ExShowScreenMessage.MIDDLE_CENTER, 3000));
					}
					
					if (_timer != null)
					{
						_timer.cancel(true);
						_timer = null;
					}
					
					_world.finishInstance(1);
				}, 3000);
			}
		}
	}
	
	private void onStatusChanged(Instance world)
	{
		final Instance instance = InstanceManager.getInstance().getInstance(world.getId());
		switch (instance.getStatus())
		{
			case 1:
			{
				break;
			}
			case 2:
			{
				if (_timer != null)
				{
					_timer.cancel(true);
				}
				
				for (Player player : _playersInside)
				{
					player.sendPacket(new ExShowScreenMessage(NpcStringId.YOU_MUST_STOP_THE_ALTAR_BEFORE_EVERYTHING_IS_SACRIFICED, ExShowScreenMessage.MIDDLE_CENTER, 6000));
				}
				
				_timer = ThreadPool.scheduleAtFixedRate(new VictimDefeatTask(5, ALTAR_TIME, world), 0, 1000);
				break;
			}
			case 3:
			{
				if (_timer != null)
				{
					_timer.cancel(true);
					_timer = null;
				}
				break;
			}
			case 4:
			{
				if (_timer != null)
				{
					_timer.cancel(true);
				}
				
				for (Player player : _playersInside)
				{
					player.sendPacket(new ExShowScreenMessage(NpcStringId.YOU_MUST_STOP_THE_ALTAR_BEFORE_EVERYTHING_IS_SACRIFICED, ExShowScreenMessage.MIDDLE_CENTER, 6000));
				}
				
				_timer = ThreadPool.scheduleAtFixedRate(new VictimDefeatTask(5, ALTAR_TIME, world), 0, 1000);
				break;
			}
			case 5:
			case 6:
			{
				if (_timer != null)
				{
					_timer.cancel(true);
					_timer = null;
				}
				break;
			}
			case 7:
			{
				ThreadPool.schedule(() -> _timer = ThreadPool.scheduleAtFixedRate(new Runnable()
				{
					private int _time = ALTAR_TIME;
					
					@Override
					public void run()
					{
						if (_time <= 0)
						{
							return;
						}
						
						boolean defeated = false;
						if (_killedMonsters.containsKey(MELISSA0) && (_killedMonsters.get(MELISSA0) == 1) && _killedMonsters.containsKey(ISADORA) && (_killedMonsters.get(ISADORA) == 1))
						{
							defeated = true;
							for (Player player : _playersInside)
							{
								player.sendPacket(new ExSendUIEvent(player, ExSendUIEvent.TYPE_NORNIL, (_time--), 0, 0, 0, 0, 2518008));
								player.sendPacket(new ExSendUIEvent(player, 0x01, 0, 0, 0, 0, 0, 2518008));
								player.sendPacket(new ExShowScreenMessage(NpcStringId.ALTAR_HAS_STOPPED, ExShowScreenMessage.MIDDLE_CENTER, 5000));
								addSpawn(MELISSA, MELISSA_SPAWN.getX(), MELISSA_SPAWN.getY(), MELISSA_SPAWN.getZ(), 0, false, 0, true, world.getId());
								if (_timer != null)
								{
									_timer.cancel(true);
									_timer = null;
								}
								
								_altar.deleteMe();
								world.setStatus(8);
							}
						}
						else
						{
							for (Player player : world.getPlayers())
							{
								if (_time == ALTAR_TIME)
								{
									player.sendPacket(new ExShowScreenMessage(NpcStringId.ALTAR_OF_SHILEN_IS_STARTING_MUST_FOCUS_FIRE_THE_ALTAR, ExShowScreenMessage.MIDDLE_CENTER, 5000));
								}
								
								player.sendPacket(new ExSendUIEvent(player, ExSendUIEvent.TYPE_NORNIL, (_time--), 0, 0, 0, 0, 2518008));
							}
						}
						
						if (!defeated && (_time == 9))
						{
							for (Player player : _playersInside)
							{
								player.sendPacket(new ExShowScreenMessage(NpcStringId.THE_ALTAR_OF_SHILEN_WILL_BECOME_ACTIVATED_IN_10_SECONDS, ExShowScreenMessage.MIDDLE_CENTER, 5000));
							}
						}
						
						if (!defeated && (_time == 4))
						{
							for (Player player : _playersInside)
							{
								player.sendPacket(new ExShowScreenMessage(NpcStringId.THE_ALTAR_OF_SHILEN_WILL_BECOME_ACTIVATED_IN_5_SECONDS, ExShowScreenMessage.MIDDLE_CENTER, 5000));
							}
						}
						
						if (!defeated && (_time <= 0))
						{
							for (Player player : _playersInside)
							{
								player.sendPacket(new ExShowScreenMessage(NpcStringId.FOCUS_FIRE_THE_ALTAR_TO_STOP_BLESSING_OF_SHILEN, ExShowScreenMessage.MIDDLE_CENTER, 5000));
							}
							
							if (isInInstance(world))
							{
								World.getInstance().forEachVisibleObjectInRange(_altar, Monster.class, 8000, boss ->
								{
									if ((boss != null) && !boss.isDead())
									{
										_altar.doCast(PROTECTED_ALTAR.getSkill());
										boss.setCurrentHp(boss.getCurrentHp() + (boss.getMaxHp() / 2));
										for (Player player : _playersInside)
										{
											player.sendPacket(new Earthquake(player.getX(), player.getY(), player.getZ(), 30, 5));
										}
									}
								});
							}
						}
						
						if ((_time <= 0) && world.isStatus(7))
						{
							ThreadPool.schedule(() -> _time = ALTAR_TIME, 15000);
						}
					}
				}, 0, 1000), 5000);
				break;
			}
		}
	}
	
	public static void main(String[] args)
	{
		new AltarOfShilen();
	}
}
