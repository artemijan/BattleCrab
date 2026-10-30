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
package ai.bosses.Baylor;

import java.util.List;

import org.l2jmobius.gameserver.managers.InstanceManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Door;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureDeath;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.SocialAction;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * Baylor Warzone instance zone.
 * @author St3eT
 */
public class BaylorWarzone extends InstanceScript
{
	// NPCs
	private static final int BAYLOR = 29213;
	private static final int PRISON_GUARD = 29104;
	private static final int ENTRANCE_PORTAL = 33523;
	private static final int INVISIBLE_NPC_1 = 29106;
	private static final int INVISIBLE_NPC_2 = 29108;
	private static final int INVISIBLE_NPC_3 = 29109;
	
	// Skills
	private static final SkillHolder INVIS_NPC_SOCIAL_SKILL = new SkillHolder(5401, 1);
	private static final SkillHolder BAYLOR_SOCIAL_SKILL = new SkillHolder(5402, 1);
	
	// Locations
	private static final Location BATTLE_PORT = new Location(153567, 143319, -12736);
	
	// Misc
	private static final int TEMPLATE_ID = 166;
	
	public BaylorWarzone()
	{
		super(TEMPLATE_ID);
		addStartNpc(ENTRANCE_PORTAL);
		addTalkId(ENTRANCE_PORTAL);
		addInstanceCreatedId(TEMPLATE_ID);
		addSpawnId(INVISIBLE_NPC_1);
		addSpellFinishedId(INVISIBLE_NPC_1);
		addCreatureSeeId(INVISIBLE_NPC_1);
		setCreatureKillId(this::onBossKill, BAYLOR);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("enterInstance"))
		{
			if (player.isInParty())
			{
				final Party party = player.getParty();
				if (!party.isLeader(player))
				{
					player.sendPacket(SystemMessageId.ONLY_A_PARTY_LEADER_CAN_MAKE_THE_REQUEST_TO_ENTER);
					return null;
				}
				
				if (player.isInCommandChannel())
				{
					player.sendPacket(SystemMessageId.YOU_CANNOT_ENTER_BECAUSE_YOU_DO_NOT_MEET_THE_REQUIREMENTS);
					return null;
				}
				
				final long currentTime = System.currentTimeMillis();
				final List<Player> members = party.getMembers();
				for (Player member : members)
				{
					if (!member.isInsideRadius3D(npc, 1000))
					{
						player.sendMessage("Player " + member.getName() + " must come closer.");
						return null;
					}
					
					if (currentTime < InstanceManager.getInstance().getInstanceTime(member, TEMPLATE_ID))
					{
						final SystemMessage msg = new SystemMessage(SystemMessageId.C1_MAY_NOT_RE_ENTER_YET);
						msg.addString(member.getName());
						party.broadcastToPartyMembers(member, msg);
						return null;
					}
				}
				
				for (Player member : members)
				{
					enterInstance(member, npc, TEMPLATE_ID);
				}
			}
			else if (player.isGM())
			{
				enterInstance(player, npc, TEMPLATE_ID);
			}
			else
			{
				player.sendPacket(SystemMessageId.YOU_ARE_NOT_CURRENTLY_IN_A_PARTY_SO_YOU_CANNOT_ENTER);
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			switch (event)
			{
				case "START_SCENE_01":
				{
					specialCamera(world, npc, 2300, 0, 90, 2000, 3000, 2500, 0, 180, 1, 0, 1);
					playSound(world, "BS06_A");
					getTimers().addTimer("START_SCENE_02", 2000, npc, null);
					break;
				}
				case "START_SCENE_02":
				{
					specialCamera(world, npc, 10, 180, 0, 17000, 3000, 30000, 0, 20, 1, 0, 0);
					getTimers().addTimer("START_SCENE_03", 17000, npc, null);
					break;
				}
				case "START_SCENE_03":
				{
					specialCamera(world, npc, 15, 15, 0, 6000, 3000, 9000, 0, 10, 1, 0, 0);
					getTimers().addTimer("START_SCENE_04", 6000, npc, null);
					break;
				}
				case "START_SCENE_04":
				{
					specialCamera(world, npc, 5, 200, 0, 6000, 3000, 9000, 0, 10, 1, 0, 0);
					getTimers().addTimer("START_SCENE_05", 6000, npc, null);
					break;
				}
				case "START_SCENE_05":
				{
					specialCamera(world, npc, 5, 70, 0, 4000, 3000, 9000, 0, 10, 1, 0, 0);
					getTimers().addTimer("START_SCENE_06", 4000, npc, null);
					break;
				}
				case "START_SCENE_06":
				{
					specialCamera(world, npc, 200, 60, 0, 1000, 3000, 4000, 0, 6, 1, 0, 0);
					getTimers().addTimer("START_SCENE_07", 1000, npc, null);
					break;
				}
				case "START_SCENE_07":
				{
					specialCamera(world, npc, 500, 260, 0, 8000, 3000, 9000, 0, 10, 1, 0, 0);
					getTimers().addTimer("SPAWN_GUARD_1", 100, e -> world.spawnGroup("PRISON_GUARD_GROUP_1"));
					getTimers().addTimer("SPAWN_GUARD_2", 2100, e -> world.spawnGroup("PRISON_GUARD_GROUP_2"));
					getTimers().addTimer("SPAWN_GUARD_3", 4100, e -> world.spawnGroup("PRISON_GUARD_GROUP_3"));
					getTimers().addTimer("SPAWN_GUARD_4", 6100, e -> world.spawnGroup("PRISON_GUARD_GROUP_4"));
					getTimers().addTimer("START_SCENE_08", 8000, npc, null);
					break;
				}
				case "START_SCENE_08":
				{
					specialCamera(world, npc, 850, 260, 70, 2000, 3000, 5000, 0, 0, 1, 0, 0);
					getTimers().addTimer("START_SCENE_09", 2000, npc, null);
					break;
				}
				case "START_SCENE_09":
				{
					specialCamera(world, npc, 0, 260, 70, 1800, 3000, 2000, 0, 0, 1, 0, 0);
					getTimers().addTimer("START_SCENE_10", 1800, npc, null);
					getTimers().addTimer("START_SCENE_12", 1000, npc, null);
					break;
				}
				case "START_SCENE_10":
				{
					specialCamera(world, npc, 200, 20, 0, 100, 3000, 5000, 160, 15, 1, 0, 0);
					getTimers().addTimer("START_SCENE_11", 2000, npc, null);
					break;
				}
				case "START_SCENE_11":
				{
					specialCamera(world, npc, 100, 20, 0, 4000, 3000, 7000, 160, 10, 1, 0, 0);
					getTimers().addTimer("START_SCENE_14", 6500, npc, null);
					break;
				}
				case "START_SCENE_12":
				{
					int count = 0;
					for (Npc baylor : world.spawnGroup("BAYLOR"))
					{
						baylor.getVariables().set("is_after_you", count);
						baylor.disableCoreAI(true);
						baylor.setRandomAnimation(false);
						baylor.setRandomWalking(false);
						baylor.asAttackable().setCanReturnToSpawnPoint(false);
						count++;
					}
					
					getTimers().addTimer("START_SCENE_13", 300, npc, null);
					break;
				}
				case "START_SCENE_13":
				{
					world.getAliveNpcs(BAYLOR).forEach(baylor ->
					{
						getTimers().addTimer("BAYLOR_SOCIAL_SKILL", (baylor.getVariables().getInt("is_after_you") == 0 ? 14 : 16) * 1000, baylor, null);
						broadcastSocialAction(baylor, 1);
					});
					break;
				}
				case "START_SCENE_14":
				{
					world.getAliveNpcs(INVISIBLE_NPC_3).forEach(invisNpc ->
					{
						specialCamera(world, invisNpc, 45, 237, 0, 0, 3000, 5000, 0, 27, 1, 0, 1);
						getTimers().addTimer("START_SCENE_15", 1500, invisNpc, null);
					});
					
					world.getAliveNpcs(PRISON_GUARD).forEach(guard ->
					{
						final int random = getRandom(100);
						if (random >= 20)
						{
							broadcastSocialAction(guard, 2);
						}
						else if (random >= 40)
						{
							getTimers().addTimer("SOCIAL_ACTION", 250, e -> broadcastSocialAction(guard, 2));
						}
						else if (random >= 60)
						{
							getTimers().addTimer("SOCIAL_ACTION", 500, e -> broadcastSocialAction(guard, 2));
						}
						else if (random >= 80)
						{
							getTimers().addTimer("SOCIAL_ACTION", 700, e -> broadcastSocialAction(guard, 2));
						}
						else
						{
							getTimers().addTimer("SOCIAL_ACTION", 800, e -> broadcastSocialAction(guard, 2));
						}
					});
					break;
				}
				case "START_SCENE_15":
				{
					world.getAliveNpcs(INVISIBLE_NPC_3).forEach(invisNpc ->
					{
						specialCamera(world, invisNpc, 500, 212, 0, 1500, 3000, 3000, 357, 15, 1, 0, 1);
						getTimers().addTimer("START_SCENE_16", 1500, invisNpc, null);
					});
					
					getTimers().addTimer("NPC_DESPAWN", 3000, e -> npc.deleteMe());
					break;
				}
				case "START_SCENE_16":
				{
					specialCamera(world, npc, 500, 212, 0, 1000, 3000, 3000, 357, 40, 1, 0, 0);
					getTimers().addTimer("START_SCENE_17", 1000, npc, null);
					break;
				}
				case "START_SCENE_17":
				{
					specialCamera(world, npc, 900, 212, 0, 1000, 3000, 3000, 357, 10, 1, 0, 0);
					getTimers().addTimer("START_SCENE_18", 2000, npc, null);
					break;
				}
				case "START_SCENE_18":
				{
					specialCamera(world, npc, 500, 212, 0, 3000, 3000, 15000, 357, 20, 1, 0, 0);
					getTimers().addTimer("START_SCENE_19", 7000, npc, null);
					break;
				}
				case "START_SCENE_19":
				{
					specialCamera(world, npc, 700, 212, 30, 1000, 7000, 2500, 357, 0, 1, 0, 0);
					break;
				}
				case "BAYLOR_SOCIAL_SKILL":
				{
					npc.doCast(BAYLOR_SOCIAL_SKILL.getSkill());
					npc.disableCoreAI(false);
					world.getAliveNpcs(INVISIBLE_NPC_1).forEach(invisNpc -> getTimers().addTimer("INVIS_NPC_SOCIAL_SKILL", 1300, invisNpc, null));
					break;
				}
				case "INVIS_NPC_SOCIAL_SKILL":
				{
					npc.doCast(INVIS_NPC_SOCIAL_SKILL.getSkill());
					break;
				}
			}
		}
	}
	
	@Override
	public void onSpellFinished(Npc npc, Player player, Skill skill)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			world.getAliveNpcs(INVISIBLE_NPC_1, INVISIBLE_NPC_2, INVISIBLE_NPC_3).forEach(Npc::deleteMe);
			world.getAliveNpcs(PRISON_GUARD).forEach(guard -> guard.doDie(null));
			npc.deleteMe();
		}
	}
	
	@Override
	public void onInstanceCreated(Instance instance, Player player)
	{
		getTimers().addTimer("BATTLE_PORT", 3000, e ->
		{
			instance.getPlayers().forEach(p -> p.teleToLocation(BATTLE_PORT));
			instance.getDoors().forEach(Door::closeMe);
		});
	}
	
	public void onBossKill(OnCreatureDeath event)
	{
		final Npc npc = event.getTarget().asNpc();
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			if (world.getAliveNpcs(BAYLOR).isEmpty())
			{
				world.finishInstance();
			}
			else
			{
				world.setReenterTime();
			}
		}
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world) && creature.isPlayer() && npc.isScriptValue(0))
		{
			npc.setScriptValue(1);
			getTimers().addTimer("START_SCENE_01", 5000, npc, null);
		}
	}
	
	/**
	 * Broadcasts SocialAction packet to self and known players.
	 * @param creature
	 * @param actionId
	 */
	private void broadcastSocialAction(Creature creature, int actionId)
	{
		final SocialAction action = new SocialAction(creature.getObjectId(), actionId);
		World.getInstance().forEachVisibleObject(creature, Player.class, player -> player.sendPacket(action));
	}
	
	public static void main(String[] args)
	{
		new BaylorWarzone();
	}
}
