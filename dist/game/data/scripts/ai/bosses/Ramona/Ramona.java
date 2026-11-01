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
package ai.bosses.Ramona;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

import org.l2jmobius.gameserver.config.GrandBossConfig;
import org.l2jmobius.gameserver.data.xml.MapRegionData;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.managers.GlobalVariablesManager;
import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.player.TeleportWhereType;
import org.l2jmobius.gameserver.model.actor.instance.Door;
import org.l2jmobius.gameserver.model.script.QuestTimer;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.variables.NpcVariables;
import org.l2jmobius.gameserver.model.zone.type.EffectZone;
import org.l2jmobius.gameserver.model.zone.type.NoSummonFriendZone;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.OnEventTrigger;
import org.l2jmobius.gameserver.util.MathUtil;

/**
 * Ramona RB
 * @author Gigi
 * @date 2017-04-09 - [10:22:38]
 */
public class Ramona extends Script
{
	// Status
	private enum Status
	{
		ALIVE,
		IN_FIGHT,
		DEAD
	}
	
	// NPC
	private static final int MP_CONTROL = 19642;
	private static final int RAMONA = 19648;
	private static final int RAMONA_1 = 26141;
	private static final int RAMONA_2 = 26142;
	private static final int RAMONA_3 = 26143;
	private static final int[] MINION_LIST =
	{
		26144, // Dancer of the Queen
		26145, // Commander of the Queen
		26146, // Shooter of the Queen
		26147 // Wizard of the Queen
	};
	
	// Trigers
	private static final int FIRST_GENERATOR = 22230702;
	private static final int SECOND_GENERATOR = 22230704;
	private static final int THRID_GENERATOR = 22230706;
	private static final int FOURTH_GENERATOR = 22230708;
	
	// Skills
	private static final Skill HYPER_MEGA_PLASMA_SHOT = SkillData.getInstance().getSkill(16641, 1);
	private static final Skill HYPER_MEGA_PLASMA_BRUST = SkillData.getInstance().getSkill(16642, 1);
	private static final Skill HIPER_MEGA_TELEKINESS = SkillData.getInstance().getSkill(16643, 1);
	private static final Skill RIDE_THE_LIGHTING = SkillData.getInstance().getSkill(16644, 1);
	private static final Skill RIDE_THE_LIGHTING_MEGA_BRUST = SkillData.getInstance().getSkill(16645, 1);
	private static final Skill ULTRA_MEGA_TELEKINESS = SkillData.getInstance().getSkill(16647, 1);
	private static final Skill[] RAMONA1_SKILLS =
	{
		HYPER_MEGA_PLASMA_BRUST,
		HYPER_MEGA_PLASMA_SHOT,
		RIDE_THE_LIGHTING
	};
	private static final Skill[] RAMONA2_SKILLS =
	{
		HYPER_MEGA_PLASMA_BRUST,
		HYPER_MEGA_PLASMA_SHOT,
		RIDE_THE_LIGHTING,
		RIDE_THE_LIGHTING_MEGA_BRUST
	};
	private static final Skill[] RAMONA3_SKILLS =
	{
		HYPER_MEGA_PLASMA_BRUST,
		HYPER_MEGA_PLASMA_SHOT,
		RIDE_THE_LIGHTING,
		RIDE_THE_LIGHTING_MEGA_BRUST,
		HIPER_MEGA_TELEKINESS,
		ULTRA_MEGA_TELEKINESS
	};
	
	// Locations
	private static final Location DEFAULT_LOC = new Location(86338, 172099, -10602, 16383);
	private static final Location RAMONA_SPAWN_LOC = new Location(86432, 171983, -10592, 16383);
	
	// Other
	private static final int ROOM_CONTROL_DOOR = 22230711;
	private static final NoSummonFriendZone ZONE = ZoneManager.getInstance().getZoneById(210108, NoSummonFriendZone.class);
	private static final EffectZone ZONE_ATTACK = ZoneManager.getInstance().getZoneById(200109, EffectZone.class);
	private static final EffectZone ZONE_DEFENCE = ZoneManager.getInstance().getZoneById(200110, EffectZone.class);
	private static final EffectZone ZONE_HP = ZoneManager.getInstance().getZoneById(200111, EffectZone.class);
	private static final EffectZone ZONE_ERADICATION = ZoneManager.getInstance().getZoneById(200112, EffectZone.class);
	
	// Vars
	private static final String RAMONA_RESPAWN_VAR = "RamonaRespawn";
	private static Status _boss = Status.ALIVE;
	private static List<Npc> _minions = new CopyOnWriteArrayList<>();
	private static int _bossStage;
	private static long _lastAction;
	private static Npc _ramona1;
	private static Npc _ramona2;
	private static Npc _ramona3;
	
	private Ramona()
	{
		addStartNpc(MP_CONTROL);
		addKillId(MP_CONTROL, RAMONA_1, RAMONA_2, RAMONA_3);
		addAttackId(MP_CONTROL, RAMONA_1, RAMONA_2, RAMONA_3);
		addSpawnId(RAMONA_1, RAMONA_2, RAMONA_3);
		addCreatureSeeId(MP_CONTROL);
		
		final long temp = GlobalVariablesManager.getInstance().getLong(RAMONA_RESPAWN_VAR, 0) - System.currentTimeMillis();
		if (temp > 0)
		{
			_boss = Status.DEAD;
			startQuestTimer("RAMONA_UNLOCK", temp, null, null);
		}
		else
		{
			addSpawn(MP_CONTROL, RAMONA_SPAWN_LOC, false, 0, false);
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "RAMONA_UNLOCK":
			{
				_boss = Status.ALIVE;
				addSpawn(MP_CONTROL, RAMONA_SPAWN_LOC, false, 0, false);
				break;
			}
			case "SPAWN_MS":
			{
				if (ZONE.getCharactersInside().size() >= GrandBossConfig.RAMONA_MIN_PLAYER)
				{
					npc.setInvul(false);
					cancelQuestTimers("SPAWN_MS");
					startQuestTimer("CHECK_ACTIVITY_TASK", 5000, null, null);
					_lastAction = System.currentTimeMillis();
				}
				break;
			}
			case "SPAWN_RAMONA_1":
			{
				_bossStage = 1;
				World.getInstance().forEachVisibleObjectInRange(npc, Npc.class, 3000, ramona ->
				{
					if (ramona.getId() == RAMONA)
					{
						ramona.deleteMe();
					}
				});
				_ramona1 = addSpawn(RAMONA_1, RAMONA_SPAWN_LOC, false, 1200000, true);
				startQuestTimer("GENERATOR_1", getRandom(300000, 600000), null, null);
				startQuestTimer("GENERATOR_2", getRandom(900000, 1200000), null, null);
				startQuestTimer("GENERATOR_3", getRandom(1500000, 1800000), null, null);
				startQuestTimer("GENERATOR_4", getRandom(2100000, 2400000), null, null);
				_lastAction = System.currentTimeMillis();
				startQuestTimer("RAMONA1_SKILL", 6000, _ramona1, null);
				break;
			}
			case "GENERATOR_1":
			{
				ZONE.broadcastPacket(new OnEventTrigger(FIRST_GENERATOR, true));
				ZONE_ATTACK.setEnabled(true);
				break;
			}
			case "GENERATOR_2":
			{
				ZONE.broadcastPacket(new OnEventTrigger(SECOND_GENERATOR, true));
				ZONE_DEFENCE.setEnabled(true);
				break;
			}
			case "GENERATOR_3":
			{
				ZONE.broadcastPacket(new OnEventTrigger(THRID_GENERATOR, true));
				ZONE_HP.setEnabled(true);
				break;
			}
			case "GENERATOR_4":
			{
				ZONE.broadcastPacket(new OnEventTrigger(FOURTH_GENERATOR, true));
				ZONE_ERADICATION.setEnabled(true);
				break;
			}
			case "SPAWN_RAMONA2":
			{
				playMovie(ZONE.getPlayersInside(), Movie.SC_RAMONA_TRANS_A);
				_ramona2 = addSpawn(RAMONA_2, RAMONA_SPAWN_LOC, false, 1200000, false);
				_ramona2.setCurrentHp(_ramona2.getMaxHp() * 0.75);
				break;
			}
			case "SPAWN_RAMONA3":
			{
				playMovie(ZONE.getPlayersInside(), Movie.SC_RAMONA_TRANS_B);
				_ramona3 = addSpawn(RAMONA_3, RAMONA_SPAWN_LOC, false, 1200000, false);
				_ramona3.setCurrentHp(_ramona3.getMaxHp() * 0.5);
				break;
			}
			case "RAMONA1_SKILL":
			{
				if ((_bossStage == 1) && _ramona1.isInCombat())
				{
					Skill randomAttackSkill = RAMONA1_SKILLS[getRandom(RAMONA1_SKILLS.length)];
					if (getRandom(100) > 20)
					{
						_ramona1.doCast(randomAttackSkill);
					}
				}
				break;
			}
			case "SPAWN_RAMONA_MINIONS":
			{
				_bossStage = 2;
				for (int i = 0; i < 7; i++)
				{
					if (_ramona2 != null)
					{
						final Npc minion = addSpawn(MINION_LIST[getRandom(MINION_LIST.length)], _ramona2.getX() + getRandom(-200, 200), _ramona2.getY() + getRandom(-200, 200), _ramona2.getZ(), _ramona2.getHeading(), false, 600000);
						minion.setRunning();
						minion.asAttackable().setIsRaidMinion(true);
						addAttackPlayerDesire(minion, player);
						_minions.add(minion);
					}
				}
				
				startQuestTimer("RAMONA2_SKILL", 6000, _ramona2, null);
				break;
			}
			case "RAMONA2_SKILL":
			{
				if ((_bossStage == 2) && _ramona2.isInCombat())
				{
					Skill randomAttackSkill = RAMONA2_SKILLS[getRandom(RAMONA2_SKILLS.length)];
					if (getRandom(100) > 20)
					{
						_ramona2.doCast(randomAttackSkill);
					}
				}
				break;
			}
			case "SPAWN_RAMONA_MINIONS_1":
			{
				_bossStage = 3;
				for (int i = 0; i < 7; i++)
				{
					if (_ramona3 != null)
					{
						final Npc minion = addSpawn(MINION_LIST[getRandom(MINION_LIST.length)], _ramona3.getX() + getRandom(-200, 200), _ramona3.getY() + getRandom(-200, 200), _ramona3.getZ(), _ramona3.getHeading(), false, 600000);
						minion.setRunning();
						minion.asAttackable().setIsRaidMinion(true);
						addAttackPlayerDesire(minion, player);
						_minions.add(minion);
					}
				}
				
				startQuestTimer("RAMONA3_SKILL", 6000, _ramona3, null);
				break;
			}
			case "RAMONA3_SKILL":
			{
				if ((_bossStage == 3) && _ramona3.isInCombat())
				{
					Skill randomAttackSkill = RAMONA3_SKILLS[getRandom(RAMONA3_SKILLS.length)];
					if (getRandom(100) > 20)
					{
						_ramona3.doCast(randomAttackSkill);
					}
				}
				break;
			}
			case "CHECK_ACTIVITY_TASK":
			{
				if ((_lastAction + 900000) < System.currentTimeMillis())
				{
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
					
					startQuestTimer("END_RAMONA", 2000, null, null);
				}
				else
				{
					startQuestTimer("CHECK_ACTIVITY_TASK", 60000, null, null);
				}
				break;
			}
			case "END_RAMONA":
			{
				_bossStage = 0;
				ZONE.oustAllPlayers();
				if (_ramona1 != null)
				{
					_ramona1.deleteMe();
				}
				
				if (_ramona2 != null)
				{
					_ramona2.deleteMe();
				}
				
				if (_ramona3 != null)
				{
					_ramona3.deleteMe();
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
				
				if ((_boss == Status.ALIVE) || (_boss == Status.IN_FIGHT))
				{
					addSpawn(MP_CONTROL, RAMONA_SPAWN_LOC, false, 0, false);
				}
				
				QuestTimer activityTimer = getQuestTimer("CHECK_ACTIVITY_TASK", null, null);
				if (activityTimer != null)
				{
					activityTimer.cancel();
				}
				
				for (int i = FIRST_GENERATOR; i <= FOURTH_GENERATOR; i++)
				{
					ZONE.broadcastPacket(new OnEventTrigger(i, false));
				}
				
				ZONE_ATTACK.setEnabled(false);
				ZONE_DEFENCE.setEnabled(false);
				ZONE_HP.setEnabled(false);
				ZONE_ERADICATION.setEnabled(false);
				cancelQuestTimers("GENERATOR_1");
				cancelQuestTimers("GENERATOR_2");
				cancelQuestTimers("GENERATOR_3");
				cancelQuestTimers("GENERATOR_4");
				addSpawn(RAMONA, DEFAULT_LOC, false, 0, false);
				_minions.clear();
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon, Skill skill)
	{
		switch (npc.getId())
		{
			case MP_CONTROL:
			{
				if (ZONE.getCharactersInside().size() < GrandBossConfig.RAMONA_MIN_PLAYER)
				{
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WHAT_S_UP_WITH_YOUR_EYES_YOU_NEED_MORE_ENERGY);
				}
				break;
			}
			case RAMONA_1:
			{
				if ((npc.getId() == RAMONA_1) && (_ramona1.getCurrentHp() <= (_ramona1.getMaxHp() * 0.75)) && (_ramona1.getCurrentHp() > (_ramona1.getMaxHp() * 0.50)))
				{
					_ramona1.deleteMe();
					startQuestTimer("SPAWN_RAMONA2", 1000, null, null);
					startQuestTimer("SPAWN_RAMONA_MINIONS", 6000, _ramona2, null);
				}
				
				_lastAction = System.currentTimeMillis();
				break;
			}
			case RAMONA_2:
			{
				if ((npc.getId() == RAMONA_2) && (_ramona2.getCurrentHp() <= (_ramona2.getMaxHp() * 0.50)))
				{
					_ramona2.deleteMe();
					startQuestTimer("SPAWN_RAMONA3", 1000, null, null);
					startQuestTimer("SPAWN_RAMONA_MINIONS_1", 6000, _ramona3, null);
				}
				
				_lastAction = System.currentTimeMillis();
				break;
			}
			case RAMONA_3:
			{
				_lastAction = System.currentTimeMillis();
				break;
			}
		}
		
		if ((npc.getId() == RAMONA_1) || (npc.getId() == RAMONA_2) || (npc.getId() == RAMONA_3))
		{
			if (skill == null)
			{
				refreshAiParams(attacker, npc, (damage * 1000));
			}
			else if (npc.getCurrentHp() < (npc.getMaxHp() * 0.25))
			{
				refreshAiParams(attacker, npc, ((damage / 3) * 100));
			}
			else if (npc.getCurrentHp() < (npc.getMaxHp() * 0.5))
			{
				refreshAiParams(attacker, npc, (damage * 20));
			}
			else if (npc.getCurrentHp() < (npc.getMaxHp() * 0.75))
			{
				refreshAiParams(attacker, npc, (damage * 10));
			}
			else
			{
				refreshAiParams(attacker, npc, ((damage / 3) * 20));
			}
			
			manageSkills(npc);
		}
	}
	
	private void refreshAiParams(Creature attacker, Npc npc, int damage)
	{
		refreshAiParams(attacker, npc, damage, damage);
	}
	
	private void refreshAiParams(Creature attacker, Npc npc, int damage, int aggro)
	{
		final int newAggroVal = damage + getRandom(3000);
		final int aggroVal = aggro + 1000;
		final NpcVariables vars = npc.getVariables();
		for (int i = 0; i < 3; i++)
		{
			if (attacker == vars.getObject("c_quest" + i, Creature.class))
			{
				if (vars.getInt("i_quest" + i) < aggroVal)
				{
					vars.set("i_quest" + i, newAggroVal);
				}
				return;
			}
		}
		
		final int index = MathUtil.getIndexOfMinValue(vars.getInt("i_quest0"), vars.getInt("i_quest1"), vars.getInt("i_quest2"));
		vars.set("i_quest" + index, newAggroVal);
		vars.set("c_quest" + index, attacker);
	}
	
	@Override
	public void onSpellFinished(Npc npc, Player player, Skill skill)
	{
		startQuestTimer("MANAGE_SKILLS", 1000, npc, null);
	}
	
	private void manageSkills(Npc npc)
	{
		if (npc.isCastingNow(SkillCaster::isAnyNormalType) || npc.isCoreAIDisabled() || !npc.isInCombat())
		{
			return;
		}
		
		final NpcVariables vars = npc.getVariables();
		for (int i = 0; i < 3; i++)
		{
			final Creature attacker = vars.getObject("c_quest" + i, Creature.class);
			if ((attacker == null) || ((npc.calculateDistance3D(attacker) > 9000) || attacker.isDead()))
			{
				vars.set("i_quest" + i, 0);
			}
		}
		
		final int index = MathUtil.getIndexOfMaxValue(vars.getInt("i_quest0"), vars.getInt("i_quest1"), vars.getInt("i_quest2"));
		final Creature player = vars.getObject("c_quest" + index, Creature.class);
		final int i2 = vars.getInt("i_quest" + index);
		if ((i2 > 0) && (getRandom(100) < 70))
		{
			vars.set("i_quest" + index, 500);
		}
		
		if ((player != null) && !player.isDead())
		{
			Skill skillToCast = RAMONA3_SKILLS[getRandom(RAMONA3_SKILLS.length)];
			if ((skillToCast != null) && SkillCaster.checkUseConditions(npc, skillToCast))
			
			{
				npc.setTarget(player);
				npc.doCast(skillToCast);
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		switch (npc.getId())
		{
			case MP_CONTROL:
			{
				World.getInstance().forEachVisibleObjectInRange(npc, Door.class, 8000, door ->
				{
					if (door.getId() == ROOM_CONTROL_DOOR)
					{
						door.closeMe();
					}
				});
				startQuestTimer("SPAWN_RAMONA_1", 6000, npc, null);
				break;
			}
			case RAMONA_3:
			{
				_boss = Status.DEAD;
				
				final long baseIntervalMillis = GrandBossConfig.RAMONA_SPAWN_INTERVAL * 3600000;
				final long randomRangeMillis = GrandBossConfig.RAMONA_SPAWN_RANDOM * 3600000;
				final long respawnTime = baseIntervalMillis + getRandom(-randomRangeMillis, randomRangeMillis);
				GlobalVariablesManager.getInstance().set(RAMONA_RESPAWN_VAR, System.currentTimeMillis() + respawnTime);
				
				startQuestTimer("RAMONA_UNLOCK", respawnTime, null, null);
				startQuestTimer("END_RAMONA", 90000, null, null);
				break;
			}
		}
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		npc.setInvul(true);
		if (creature.isPlayer())
		{
			startQuestTimer("SPAWN_MS", 10000, npc, null, true);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		switch (npc.getId())
		{
			case RAMONA_1:
			{
				_boss = Status.IN_FIGHT;
				_lastAction = System.currentTimeMillis();
				break;
			}
			case RAMONA_2:
			case RAMONA_3:
			{
				_lastAction = System.currentTimeMillis();
				break;
			}
		}
	}
	
	public static void main(String[] args)
	{
		new Ramona();
	}
}
