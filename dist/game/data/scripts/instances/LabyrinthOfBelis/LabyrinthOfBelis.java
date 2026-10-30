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
package instances.LabyrinthOfBelis;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Attackable;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureDeath;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.Q10331_StartOfFate.Q10331_StartOfFate;

/**
 * Labyrinth of Belis Instance Zone.
 * @author Gladicek, Mobius
 */
public class LabyrinthOfBelis extends InstanceScript
{
	// NPCs
	private static final int SEBION = 32972;
	private static final int INFILTRATION_OFFICER = 19155;
	private static final int BELIS_VERITIFICATION_SYSTEM = 33215;
	private static final int OPERATIVE = 22998;
	private static final int HANDYMAN = 22997;
	private static final int ELECTRICITY_GENERATOR = 33216;
	private static final int NEMERTESS = 22984;
	
	// Items
	// private static final int SARIL_NECKLACE = 17580;
	private static final int BELIS_MARK = 17615;
	
	// Skills
	private static final SkillHolder CURRENT_SHOCK = new SkillHolder(14698, 1);
	
	// Locations
	private static final Location INFILTRATION_OFFICER_ROOM_2 = new Location(-117040, 212502, -8592);
	private static final Location INFILTRATION_OFFICER_ROOM_3 = new Location(-117843, 214230, -8592);
	private static final Location INFILTRATION_OFFICER_ROOM_3_INSIDE = new Location(-118248, 214676, -8590);
	private static final Location INFILTRATION_OFFICER_ROOM_4 = new Location(-119217, 213743, -8600);
	private static final Location SPAWN_ATTACKERS = new Location(-116809, 213275, -8606);
	private static final Location GENERATOR_SPAWN = new Location(-118333, 214791, -8557);
	private static final Location ATTACKER_SPOT = new Location(-117927, 214391, -8600);
	private static final Location NEMERTESS_SPAWN = new Location(-118336, 212973, -8680);
	
	// Misc
	private static final int TEMPLATE_ID = 178;
	private static final int DOOR_ID_ROOM_1_2 = 16240002;
	private static final int DOOR_ID_ROOM_2_1 = 16240003;
	private static final int DOOR_ID_ROOM_2_2 = 16240004;
	private static final int DOOR_ID_ROOM_3_1 = 16240005;
	private static final int DOOR_ID_ROOM_3_2 = 16240006;
	private static final int DOOR_ID_ROOM_4_1 = 16240007;
	private static final int DOOR_ID_ROOM_4_2 = 16240008;
	private static final int DAMAGE_ZONE = 12014;
	
	public LabyrinthOfBelis()
	{
		super(TEMPLATE_ID);
		addStartNpc(SEBION, INFILTRATION_OFFICER, BELIS_VERITIFICATION_SYSTEM);
		addFirstTalkId(INFILTRATION_OFFICER, ELECTRICITY_GENERATOR, BELIS_VERITIFICATION_SYSTEM);
		addTalkId(SEBION, INFILTRATION_OFFICER, BELIS_VERITIFICATION_SYSTEM);
		addSpawnId(INFILTRATION_OFFICER);
		addAttackId(INFILTRATION_OFFICER);
		addMoveFinishedId(INFILTRATION_OFFICER);
		addKillId(OPERATIVE, HANDYMAN, INFILTRATION_OFFICER, NEMERTESS);
		addEnterZoneId(DAMAGE_ZONE);
		addExitZoneId(DAMAGE_ZONE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("enter_instance"))
		{
			enterInstance(player, npc, TEMPLATE_ID);
		}
		else
		{
			final Instance world = npc.getInstanceWorld();
			if (world != null)
			{
				switch (event)
				{
					case "room1":
					{
						if (world.isStatus(0))
						{
							npc.setScriptValue(1);
							npc.getAI().startFollow(player);
							
							world.setStatus(1);
							world.spawnGroup("operatives");
							world.openCloseDoor(DOOR_ID_ROOM_1_2, true);
						}
						break;
					}
					case "room2":
					{
						if (world.isStatus(3))
						{
							world.setStatus(4);
							world.openCloseDoor(DOOR_ID_ROOM_2_2, true);
							npc.setScriptValue(1);
							npc.getAI().startFollow(player);
							
							showOnScreenMsg(player, NpcStringId.MARK_OF_BELIS_CAN_BE_ACQUIRED_FROM_ENEMIES_NUSE_THEM_IN_THE_BELIS_VERIFICATION_SYSTEM, ExShowScreenMessage.TOP_CENTER, 4500);
							getTimers().addRepeatingTimer("MESSAGE", 10000, npc, player);
						}
						break;
					}
					case "room3":
					{
						if (world.isStatus(5))
						{
							world.setStatus(6);
							world.openCloseDoor(DOOR_ID_ROOM_3_2, true);
							
							final Npc generator = addSpawn(ELECTRICITY_GENERATOR, GENERATOR_SPAWN, false, 0, false, world.getId());
							generator.disableCoreAI(true);
							
							npc.setScriptValue(1);
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DON_T_COME_BACK_HERE);
							npc.setTarget(generator);
							npc.getAI().setIntention(Intention.MOVE_TO, INFILTRATION_OFFICER_ROOM_3_INSIDE);
							getTimers().addTimer("GENERATOR_EFFECT", 300, generator, null);
							getTimers().addRepeatingTimer("MESSAGE", 7000, npc, null);
							getTimers().addRepeatingTimer("ATTACKERS", 12500, npc, player);
							npc.asAttackable().addDamageHate(generator, 0, 9999); // TODO: Find better way for attack
							npc.reduceCurrentHp(1, generator, null);
						}
						break;
					}
					case "room4":
					{
						if (world.isStatus(7))
						{
							world.setStatus(8);
							world.openCloseDoor(DOOR_ID_ROOM_4_2, true);
							npc.setScriptValue(1);
							playMovie(player, Movie.SC_TALKING_ISLAND_BOSS_OPENING);
							getTimers().addTimer("SPAWN_NEMERTESS", 50000, npc, null);
						}
						break;
					}
					case "giveBelisMark":
					{
						if (world.isStatus(4))
						{
							if (hasAtLeastOneQuestItem(player, BELIS_MARK))
							{
								takeItems(player, BELIS_MARK, 1);
								
								switch (npc.getScriptValue())
								{
									case 0:
									{
										npc.setScriptValue(1);
										return "33215-01.html";
									}
									case 1:
									{
										npc.setScriptValue(2);
										return "33215-02.html";
									}
									case 2:
									{
										world.setStatus(5);
										getTimers().addTimer("ROOM_2_DONE", 500, npc, null);
										return "33215-03.html";
									}
								}
							}
							
							return "33215-04.html";
						}
						
						return "33215-05.html";
					}
					case "finish":
					{
						world.finishInstance(0);
						break;
					}
				}
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onEnterZone(Creature creature, ZoneType zone)
	{
		// TODO: Replace me with effect zone when support for instances are done.
		if (creature.isPlayer())
		{
			final Player player = creature.asPlayer();
			final Instance world = player.getInstanceWorld();
			if ((world != null) && world.isStatus(6))
			{
				getTimers().addRepeatingTimer("DEBUFF", 1500, world.getNpc(ELECTRICITY_GENERATOR), player);
			}
		}
	}
	
	@Override
	public void onExitZone(Creature creature, ZoneType zone)
	{
		// TODO: Replace me with effect zone when support for instances are done.
		if (creature.isPlayer())
		{
			final Player player = creature.asPlayer();
			final Instance world = player.getInstanceWorld();
			if ((world != null) && (world.isStatus(6) || world.isStatus(7)))
			{
				getTimers().cancelTimer("DEBUFF", world.getNpc(ELECTRICITY_GENERATOR), player);
			}
		}
	}
	
	@Override
	public void onMoveFinished(Npc npc)
	{
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			switch (world.getStatus())
			{
				case 3:
				{
					npc.setScriptValue(0);
					npc.broadcastInfo();
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HEY_YOU_RE_NOT_ALL_BAD_LET_ME_KNOW_WHEN_YOU_RE_READY);
					npc.setHeading(npc.getHeading() + 32500);
					break;
				}
				case 5:
				{
					npc.setScriptValue(0);
					npc.broadcastInfo();
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.READY_LET_ME_KNOW);
					npc.setHeading(npc.getHeading() + 32500);
					break;
				}
				case 7:
				{
					npc.abortAttack();
					npc.setScriptValue(0);
					npc.broadcastInfo();
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.SOMETHING_OMINOUS_IN_THERE_I_HOPE_YOU_RE_REALLY_READY_FOR_THIS_LET_ME_KNOW);
					npc.setHeading(npc.getHeading() + 32500);
					break;
				}
				case 9:
				{
					npc.setScriptValue(0);
					npc.setHeading(npc.getHeading() + 32500);
					break;
				}
			}
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final Instance world = npc.getInstanceWorld();
		if (world == null)
		{
			return null;
		}
		
		String htmltext = null;
		switch (npc.getId())
		{
			case INFILTRATION_OFFICER:
			{
				if (npc.isScriptValue(0))
				{
					switch (world.getStatus())
					{
						case 0:
						{
							htmltext = "19155-01.html";
							break;
						}
						case 3:
						{
							htmltext = "19155-03.html";
							break;
						}
						case 5:
						{
							htmltext = "19155-04.html";
							break;
						}
						case 7:
						{
							htmltext = "19155-05.html";
							break;
						}
						case 9:
						{
							htmltext = "19155-06.html";
							break;
						}
					}
				}
				else
				{
					htmltext = "19155-02.html";
					break;
				}
				break;
			}
			case BELIS_VERITIFICATION_SYSTEM:
			{
				htmltext = "33215.html";
				break;
			}
			case ELECTRICITY_GENERATOR:
			{
				htmltext = "33216.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			switch (npc.getId())
			{
				case OPERATIVE:
				{
					if (world.isStatus(1))
					{
						if (world.getAliveNpcs(OPERATIVE).isEmpty())
						{
							world.setStatus(2);
							getTimers().addTimer("ROOM_1_DONE", 500, npc, null);
						}
					}
					else if (world.isStatus(6) && npc.isScriptValue(1))
					{
						final int counter = world.getParameters().getInt("counter", 0);
						if (counter == 6)
						{
							getTimers().addTimer("ROOM_3_DONE", 2000, npc, player);
						}
					}
					break;
				}
				case HANDYMAN:
				{
					if (world.isStatus(4))
					{
						if (getRandom(100) > 10)
						{
							npc.dropItem(player, BELIS_MARK, 1);
						}
					}
					else if (world.isStatus(6) && npc.isScriptValue(1))
					{
						final int counter = world.getParameters().getInt("counter", 0);
						if (counter == 6)
						{
							getTimers().addTimer("ROOM_3_DONE", 2000, npc, player);
						}
					}
					break;
				}
				case INFILTRATION_OFFICER:
				{
					world.setStatus(-1);
					world.finishInstance(1);
					break;
				}
				case NEMERTESS:
				{
					final QuestState qs = player.getQuestState(Q10331_StartOfFate.class.getSimpleName());
					if (qs.isCond(1))
					{
						qs.setCond(2, true);
					}
					
					npc.deleteMe();
					playMovie(player, Movie.SC_TALKING_ISLAND_BOSS_ENDING);
					getTimers().addTimer("ROOM_4_DONE", 30000, npc, null);
					break;
				}
			}
		}
	}
	
	@RegisterEvent(EventType.ON_CREATURE_DEATH)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(INFILTRATION_OFFICER)
	public void onCreatureKill(OnCreatureDeath event)
	{
		final Npc npc = event.getTarget().asNpc();
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			world.setStatus(-1);
			world.finishInstance(1);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Attackable officer = npc.asAttackable();
		officer.setRunning();
		officer.setCanReturnToSpawnPoint(false);
		getTimers().addRepeatingTimer("MESSAGE", 6000, npc, null);
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			switch (event)
			{
				case "MESSAGE":
				{
					switch (world.getStatus())
					{
						case 0:
						{
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.LET_ME_KNOW_WHEN_YOU_RE_ALL_READY);
							break;
						}
						case 4:
						{
							showOnScreenMsg(player, NpcStringId.MARK_OF_BELIS_CAN_BE_ACQUIRED_FROM_ENEMIES_NUSE_THEM_IN_THE_BELIS_VERIFICATION_SYSTEM, ExShowScreenMessage.TOP_CENTER, 4500);
							break;
						}
						case 6:
						{
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DON_T_COME_BACK_HERE);
							break;
						}
						default:
						{
							getTimers().cancelTimer("MESSAGE", npc, null);
							break;
						}
					}
					break;
				}
				case "ATTACKERS":
				{
					if (world.isStatus(6))
					{
						final int counter = world.getParameters().getInt("counter", 0) + 1;
						if (counter == 6)
						{
							getTimers().cancelTimer("ATTACKERS", npc, player);
						}
						
						world.setParameter("counter", counter);
						showOnScreenMsg(player, (getRandomBoolean() ? NpcStringId.IF_TERAIN_DIES_THE_MISSION_WILL_FAIL : NpcStringId.BEHIND_YOU_THE_ENEMY_IS_AMBUSHING_YOU), ExShowScreenMessage.TOP_CENTER, 4500);
						final Attackable mob = addSpawn((getRandomBoolean() ? OPERATIVE : HANDYMAN), SPAWN_ATTACKERS, false, 0, true, world.getId()).asAttackable();
						mob.setRunning();
						mob.getAI().setIntention(Intention.MOVE_TO, ATTACKER_SPOT);
						mob.broadcastSay(ChatType.NPC_GENERAL, (getRandomBoolean() ? NpcStringId.KILL_THE_GUY_MESSING_WITH_THE_ELECTRIC_DEVICE : NpcStringId.FOCUS_ON_ATTACKING_THE_GUY_IN_THE_ROOM));
						mob.addDamageHate(npc, 0, 9999);
						mob.reduceCurrentHp(1, npc, null); // TODO: Find better way for attack
						mob.setScriptValue(1);
					}
					else
					{
						getTimers().cancelTimer("ATTACKERS", npc, player);
					}
					break;
				}
				case "ROOM_1_DONE":
				{
					if (world.isStatus(2))
					{
						world.setStatus(3);
						world.openCloseDoor(DOOR_ID_ROOM_2_1, true);
						
						final Npc officer = world.getNpc(INFILTRATION_OFFICER);
						officer.getAI().setIntention(Intention.MOVE_TO, INFILTRATION_OFFICER_ROOM_2);
						officer.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ALL_RIGHT_LET_S_MOVE_OUT);
					}
					break;
				}
				case "ROOM_2_DONE":
				{
					world.openCloseDoor(DOOR_ID_ROOM_3_1, true);
					
					final Npc officer = world.getNpc(INFILTRATION_OFFICER);
					officer.getAI().setIntention(Intention.MOVE_TO, INFILTRATION_OFFICER_ROOM_3);
					officer.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.COME_ON_ONTO_THE_NEXT_PLACE);
					break;
				}
				case "ROOM_3_DONE":
				{
					if (world.isStatus(6))
					{
						world.setStatus(7);
						world.openCloseDoor(DOOR_ID_ROOM_4_1, true);
						showOnScreenMsg(player, NpcStringId.ELECTRONIC_DEVICE_HAS_BEEN_DESTROYED, ExShowScreenMessage.TOP_CENTER, 4500);
						
						final Npc generator = world.getNpc(ELECTRICITY_GENERATOR);
						final Npc officer = world.getNpc(INFILTRATION_OFFICER);
						generator.doDie(officer);
						generator.deleteMe();
						getTimers().addTimer("MOVE_TO_ROOM_4", 3000, officer, null);
					}
					break;
				}
				case "MOVE_TO_ROOM_4":
				{
					npc.setRunning();
					npc.getAI().setIntention(Intention.MOVE_TO, INFILTRATION_OFFICER_ROOM_4);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DEVICE_DESTROYED_LET_S_GO_ONTO_THE_NEXT);
					break;
				}
				case "ROOM_4_DONE":
				{
					if (world.isStatus(8))
					{
						world.setStatus(9);
						
						final Npc officer = world.getNpc(INFILTRATION_OFFICER);
						officer.getAI().setIntention(Intention.MOVE_TO, NEMERTESS_SPAWN);
					}
					break;
				}
				case "GENERATOR_EFFECT":
				{
					npc.setDisplayEffect(1);
					break;
				}
				case "SPAWN_NEMERTESS":
				{
					addSpawn(NEMERTESS, NEMERTESS_SPAWN, false, 0, false, world.getId());
					break;
				}
				case "DEBUFF":
				{
					CURRENT_SHOCK.getSkill().applyEffects(npc, player);
					break;
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new LabyrinthOfBelis();
	}
}
