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
package ai.areas.Wastelands;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Attackable;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.FriendlyNpc;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureDeath;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

/**
 * Wastelands AI.
 * @author St3eT
 */
public class Wastelands extends Script
{
	// NPCs
	private static final int JOEL = 33516;
	private static final int SCHUAZEN = 33517;
	private static final int COMMANDER = 33434;
	private static final int GUARD = 33437;
	private static final int DECO_GUARD = 19140; // Antharas Decoration 5
	private static final int DECO_GUARD2 = 19163; // Wastelands Decoration 1
	private static final int REGENERATED_KANILOV = 27459;
	private static final int REGENERATED_POSLOF = 27460;
	private static final int SAKUM = 27453;
	private static final int COMMANDO = 19126;
	private static final int COMMANDO_CAPTAIN = 19127;
	
	// Misc
	private static final NpcStringId[] GUARD_SHOUT =
	{
		NpcStringId.ATTACK_2,
		NpcStringId.FOLLOW_ME_3
	};
	
	// Locations
	private static final Location GUARD_POSLOF_LOC = new Location(-29474, 187083, -3912);
	private static final Location[] COMMANDO_SAKUM_LOC =
	{
		new Location(-36525, 192032, -3640),
		new Location(-36160, 191912, -3640),
		new Location(-36371, 191370, -3632),
		new Location(-36765, 191759, -3632),
	};
	private static final Location[] COMMANDO_CAPTAIN_SAKUM_LOC =
	{
		new Location(-36683, 191475, -3632),
		new Location(-36131, 191574, -3632),
	};
	
	private Wastelands()
	{
		addKillId(REGENERATED_POSLOF, SAKUM);
		addSpawnId(COMMANDER, GUARD, DECO_GUARD, REGENERATED_KANILOV, REGENERATED_POSLOF, SAKUM);
		addCreatureSeeId(JOEL, SCHUAZEN, COMMANDO, COMMANDO_CAPTAIN);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if ((npc == null) || !npc.isSpawned())
		{
			return null;
		}
		
		switch (event)
		{
			case "SOCIAL_SHOW":
			{
				npc.broadcastSocialAction(4);
				npc.broadcastSay(ChatType.NPC_GENERAL, GUARD_SHOUT[getRandom(2)], 1000);
				World.getInstance().getVisibleObjectsInRange(npc, Npc.class, 500).stream().filter(n -> n.getId() == GUARD).forEach(guard -> startQuestTimer("SOCIAL_ACTION", getRandom(2500, 3500), guard, null));
				break;
			}
			case "SOCIAL_ACTION":
			{
				npc.broadcastSocialAction(4);
				break;
			}
			case "START_ATTACK":
			{
				final Attackable guard = npc.asAttackable();
				final int attackId;
				
				switch (guard.getId())
				{
					case JOEL:
					{
						attackId = REGENERATED_KANILOV;
						break;
					}
					case SCHUAZEN:
					{
						attackId = REGENERATED_POSLOF;
						break;
					}
					case COMMANDO:
					case COMMANDO_CAPTAIN:
					{
						attackId = SAKUM;
						break;
					}
					default:
					{
						attackId = 0;
						break;
					}
				}
				
				if (attackId > 0)
				{
					// @formatter:off
					final Monster monster = World.getInstance().getVisibleObjectsInRange(guard, Monster.class, 1000)
						.stream()
						.filter(obj -> (obj.getId() == attackId))
						.findFirst()
						.orElse(null);
					// @formatter:on
					
					if (monster != null)
					{
						World.getInstance().forEachVisibleObjectInRange(guard, Npc.class, 1000, chars ->
						{
							if (chars.getId() == attackId)
							{
								addAttackDesire(guard, chars);
								return;
							}
						});
						
						if ((guard.getId() != COMMANDO) && (guard.getId() != COMMANDO_CAPTAIN))
						{
							guard.setInvul(true);
						}
						
						if (guard.getId() == SCHUAZEN)
						{
							// @formatter:off
							final FriendlyNpc decoGuard = World.getInstance().getVisibleObjectsInRange(guard, FriendlyNpc.class, 500)
								.stream()
								.filter(obj -> (obj.getId() == DECO_GUARD2))
								.findFirst()
								.orElse(null);
							// @formatter:on
							
							if (decoGuard != null)
							{
								decoGuard.setInvul(true);
							}
						}
					}
					else
					{
						startQuestTimer("START_ATTACK", 5000, guard, null);
					}
				}
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer() && (npc.getAI().getIntention() != Intention.ATTACK))
		{
			startQuestTimer("START_ATTACK", 250, npc, null);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		switch (npc.getId())
		{
			case COMMANDER:
			{
				startQuestTimer("SOCIAL_SHOW", 13000, npc, null, true);
				npc.setRandomAnimation(false);
				break;
			}
			case REGENERATED_KANILOV:
			case REGENERATED_POSLOF:
			{
				final int guardId = npc.getId() == REGENERATED_KANILOV ? JOEL : SCHUAZEN;
				// @formatter:off
				final FriendlyNpc guard =  World.getInstance().getVisibleObjectsInRange(npc, FriendlyNpc.class, 500)
					.stream()
					.filter(obj -> (obj.getId() == guardId))
					.findFirst()
					.orElse(null);
				// @formatter:on
				
				if (guard != null)
				{
					if (guard.getId() == SCHUAZEN)
					{
						addSpawn(DECO_GUARD2, GUARD_POSLOF_LOC);
					}
					
					guard.broadcastSay(ChatType.NPC_GENERAL, guard.getId() == JOEL ? NpcStringId.AH_REGENERATOR_POSLOF_APPEARED_AGAIN : NpcStringId.AH_REGENERATOR_KANILOV_APPEARED_AGAIN);
					notifyEvent("START_ATTACK", guard, null);
				}
				break;
			}
			case SAKUM:
			{
				manageCommando(npc.asAttackable());
				break;
			}
			case GUARD:
			case DECO_GUARD:
			{
				npc.setRandomAnimation(false);
				break;
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (npc.getId() == REGENERATED_POSLOF)
		{
			World.getInstance().forEachVisibleObjectInRange(npc, Attackable.class, 1000, guard ->
			{
				if ((guard.getId() == DECO_GUARD2))
				{
					guard.deleteMe();
				}
			});
		}
		else if (npc.getId() == SAKUM)
		{
			World.getInstance().forEachVisibleObjectInRange(npc, Attackable.class, 1000, guard ->
			{
				if ((guard.getId() == COMMANDO) || (guard.getId() == COMMANDO_CAPTAIN))
				{
					guard.deleteMe();
				}
			});
		}
	}
	
	@RegisterEvent(EventType.ON_CREATURE_DEATH)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(COMMANDO)
	@Id(COMMANDO_CAPTAIN)
	public void onCreatureKill(OnCreatureDeath event)
	{
		final Attackable guard = event.getTarget().asAttackable();
		
		// @formatter:off
		final Attackable sakum = World.getInstance().getVisibleObjectsInRange(guard, Attackable.class, 1000)
			.stream()
			.filter(obj -> (obj.getId() == SAKUM))
			.findFirst()
			.orElse(null);
		// @formatter:on
		
		if (sakum != null)
		{
			manageCommando(sakum);
		}
	}
	
	private void manageCommando(Attackable sakum)
	{
		int guardCount = sakum.getVariables().getInt("GUARD_COUNT", 0);
		guardCount--;
		
		if (guardCount <= 0)
		{
			if (sakum.getVariables().getBoolean("GUARD_CAPTAIN", false))
			{
				sakum.getVariables().set("GUARD_COUNT", COMMANDO_CAPTAIN_SAKUM_LOC.length);
				sakum.getVariables().set("GUARD_CAPTAIN", false);
				for (Location loc : COMMANDO_CAPTAIN_SAKUM_LOC)
				{
					final Attackable commander = addSpawn(COMMANDO_CAPTAIN, loc).asAttackable();
					ThreadPool.schedule(() ->
					{
						if (sakum.isSpawned() && !sakum.isDead() && commander.isSpawned() && !commander.isDead())
						{
							commander.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HOW_DARE_YOU_ATTACK);
							commander.reduceCurrentHp(1, sakum, null); // TODO: Find better way for attack
							sakum.reduceCurrentHp(1, commander, null);
							notifyEvent("START_ATTACK", commander, null);
						}
					}, 1500);
				}
			}
			else
			{
				sakum.getVariables().set("GUARD_COUNT", COMMANDO_SAKUM_LOC.length);
				sakum.getVariables().set("GUARD_CAPTAIN", true);
				for (Location loc : COMMANDO_SAKUM_LOC)
				{
					final Attackable commander = addSpawn(COMMANDO, loc).asAttackable();
					ThreadPool.schedule(() ->
					{
						if (sakum.isSpawned() && !sakum.isDead() && commander.isSpawned() && !commander.isDead())
						{
							commander.reduceCurrentHp(1, sakum, null); // TODO: Find better way for attack
							sakum.reduceCurrentHp(1, commander, null);
							notifyEvent("START_ATTACK", commander, null);
						}
					}, 1500);
				}
			}
		}
		else
		{
			sakum.getVariables().set("GUARD_COUNT", guardCount);
		}
	}
	
	public static void main(String[] args)
	{
		new Wastelands();
	}
}
