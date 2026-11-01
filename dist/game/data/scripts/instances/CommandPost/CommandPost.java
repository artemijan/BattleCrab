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
package instances.CommandPost;

import java.util.List;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.managers.InstanceManager;
import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.zone.type.ScriptZone;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * @author NasSeKa, Mobius
 */
public class CommandPost extends InstanceScript
{
	// NPCs
	private static final int DEVIANNE = 34089;
	private static final int GEORK = 26135;
	private static final int BURNSTEIN = 26136;
	private static final int ADOLPH = 23590;
	private static final int BARTON = 23591;
	private static final int HAYUK = 23592;
	private static final int ELISE = 23593;
	private static final int ELRYAH = 23594;
	private static final int[] FIRST_FLOOR =
	{
		23595, // Fortress Raider
		23596, // Fortress Guardian Captain
		23597, // Atelia Passionate Soldier
		23600, // Atelia Flame Master
	};
	private static final int[] WALKING_MONSTERS =
	{
		23590, // Adolph - Brainwashed Aden Vanguard
		23591, // Barton - Brainwashed Aden Vanguard
		23592, // Hayuk - Brainwashed Aden Vanguard
		23593, // Elise - Brainwashed Aden Vanguard
		23594, // Eliyah - Brainwashed Aden Vanguard
		23605, // Aden Elite Knight - Brainwashed
		23606, // Aden Elite Warrior - Brainwashed
		23607, // Aden Elite Archer - Brainwashed
		23608, // Aden Elite Wizard - Brainwashed
		23610, // Corrupted High Priest - Embryo
		23612, // Elite Priest - Embryo
		23613, // Elite Instructor - Embryo
		23614, // Elite Executioner - Embryo
		23615, // Elite Shaman - Embryo
	};
	
	// Items
	// private static final int EMERGENCY_WHISTLE = 46404;
	// Location
	private static final Location FLOOR_2_SPAWN = new Location(-44037, 44009, -8097);
	private static final Location FLOOR_3_SPAWN = new Location(-44035, 45439, -6971);
	private static final Location GEORK_FLOOR_2_SPAWN = new Location(-44035, 45365, -8031);
	private static final Location GROUP_1_MOVE = new Location(-43540, 44519, -8097);
	private static final Location GROUP_2_MOVE = new Location(-43514, 44116, -8097);
	private static final Location GROUP_3_MOVE = new Location(-44532, 44510, -8097);
	private static final Location GROUP_4_MOVE = new Location(-44532, 44109, -8097);
	private static final Location ADOLPH_MOVE = new Location(-44020, 45085, -8097);
	private static final Location BARTON_MOVE = new Location(-43922, 44994, -8097);
	private static final Location HAYUK_MOVE = new Location(-44014, 44998, -8097);
	private static final Location ELISE_MOVE = new Location(-44120, 44999, -8097);
	private static final Location ELRYAH_MOVE = new Location(-44072, 45004, -8097);
	
	// Zones
	private static final ScriptZone FLOOR_1_TP = ZoneManager.getInstance().getZoneById(25901, ScriptZone.class);
	private static final ScriptZone FLOOR_2_TP = ZoneManager.getInstance().getZoneById(25902, ScriptZone.class);
	
	// Misc
	private static final int TEMPLATE_ID = 259;
	
	public CommandPost()
	{
		super(TEMPLATE_ID);
		addStartNpc(DEVIANNE);
		addTalkId(DEVIANNE);
		addMoveFinishedId(WALKING_MONSTERS);
		addKillId(BURNSTEIN);
		addInstanceLeaveId(TEMPLATE_ID);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "enterInstance":
			{
				final Party party = player.getParty();
				if (player.isInParty())
				{
					final long currentTime = System.currentTimeMillis();
					
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
					
					final List<Player> members = party.getMembers();
					for (Player member : members)
					{
						if (!member.isInsideRadius3D(npc, 1000))
						{
							player.sendMessage("Player " + member.getName() + " must go closer to Gatekeeper Spirit.");
							return null;
						}
						
						if (currentTime < InstanceManager.getInstance().getInstanceTime(member, TEMPLATE_ID))
						{
							final SystemMessage msg = new SystemMessage(SystemMessageId.YOU_HAVE_ENTERED_ANOTHER_INSTANT_ZONE_THEREFORE_YOU_CANNOT_ENTER_CORRESPONDING_DUNGEON);
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
				
				if (player.getInstanceWorld() != null)
				{
					startQuestTimer("check_status", 3000, null, player);
				}
				break;
			}
			case "check_status":
			{
				final Instance world = player.getInstanceWorld();
				if (!isInInstance(world))
				{
					return null;
				}
				
				switch (world.getStatus())
				{
					case 0:
					{
						world.setStatus(1);
						world.spawnGroup("geork");
						if (world.getNpc(GEORK) != null)
						{
							world.getNpc(GEORK).setInvul(true);
							world.getNpc(GEORK).setImmobilized(true);
							world.getNpc(GEORK).setRandomWalking(false);
							world.getNpc(GEORK).setTargetable(false);
						}
						
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
					case 1:
					{
						if (world.getAliveNpcCount(FIRST_FLOOR) == 0)
						{
							showOnScreenMsg(world, NpcStringId.THE_TELEPORT_GATE_TO_THE_2ND_FLOOR_HAS_BEEN_ACTIVATED, ExShowScreenMessage.TOP_CENTER, 2000, true);
							world.getNpc(GEORK).teleToLocation(GEORK_FLOOR_2_SPAWN);
							world.setStatus(2);
							for (Npc monster : world.spawnGroup("group_1"))
							{
								monster.setInvul(true);
								monster.setImmobilized(true);
								monster.setRandomWalking(false);
								monster.setTargetable(false);
								monster.setScriptValue(1);
							}
							
							for (Npc monster : world.spawnGroup("group_2"))
							{
								monster.setInvul(true);
								monster.setImmobilized(true);
								monster.setRandomWalking(false);
								monster.setTargetable(false);
							}
							
							for (Npc monster : world.spawnGroup("group_3"))
							{
								monster.setInvul(true);
								monster.setImmobilized(true);
								monster.setRandomWalking(false);
								monster.setTargetable(false);
							}
							
							for (Npc monster : world.spawnGroup("group_4"))
							{
								monster.setInvul(true);
								monster.setImmobilized(true);
								monster.setRandomWalking(false);
								monster.setTargetable(false);
							}
						}
						
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
					case 2:
					{
						int teleported = world.getParameters().getInt("TELEPORTED", 0);
						for (Player member : world.getPlayers())
						{
							if (!member.isTeleporting() && FLOOR_1_TP.isInsideZone(member))
							{
								member.teleToLocation(FLOOR_2_SPAWN);
								teleported++;
							}
						}
						
						if (teleported == world.getPlayersCount())
						{
							world.setStatus(3);
							world.setParameter("TELEPORTED", 0);
							world.openCloseDoor(world.getTemplateParameters().getInt("firstGroupId"), true);
						}
						else
						{
							world.setParameter("TELEPORTED", teleported);
						}
						
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
					case 3:
					{
						for (Npc monster : world.getNpcsOfGroup("group_1"))
						{
							monster.setImmobilized(false);
							monster.setWalking();
							monster.getAI().moveTo(GROUP_1_MOVE);
						}
						
						world.setStatus(4);
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
					case 4:
					{
						if (world.getAliveNpcCount() == 19)
						{
							for (Npc monster : world.getNpcsOfGroup("group_2"))
							{
								monster.setImmobilized(false);
								monster.setWalking();
								monster.getAI().moveTo(GROUP_2_MOVE);
							}
							
							world.openCloseDoor(world.getTemplateParameters().getInt("secondGroupId"), true);
							world.setStatus(5);
						}
						
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
					case 5:
					{
						if (world.getAliveNpcCount() == 14)
						{
							for (Npc monster : world.getNpcsOfGroup("group_3"))
							{
								monster.setImmobilized(false);
								monster.setWalking();
								monster.getAI().moveTo(GROUP_3_MOVE);
							}
							
							world.openCloseDoor(world.getTemplateParameters().getInt("thirdGroupId"), true);
							world.setStatus(6);
						}
						
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
					case 6:
					{
						if (world.getAliveNpcCount() == 8)
						{
							for (Npc monster : world.getNpcsOfGroup("group_4"))
							{
								monster.setImmobilized(false);
								monster.setWalking();
								monster.getAI().moveTo(GROUP_4_MOVE);
							}
							
							world.openCloseDoor(world.getTemplateParameters().getInt("fourthGroupId"), true);
							world.setStatus(7);
						}
						
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
					case 7:
					{
						if (world.getAliveNpcCount() == 1)
						{
							showOnScreenMsg(world, NpcStringId.THE_TELEPORT_GATE_TO_THE_3RD_FLOOR_HAS_BEEN_ACTIVATED, ExShowScreenMessage.TOP_CENTER, 2000, true);
							world.getNpc(GEORK).deleteMe();
							world.setStatus(8);
						}
						
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
					case 8:
					{
						int teleported = world.getParameters().getInt("TELEPORTED", 0);
						for (Player member : world.getPlayers())
						{
							if (!member.isTeleporting() && FLOOR_2_TP.isInsideZone(member))
							{
								member.teleToLocation(FLOOR_3_SPAWN);
								teleported++;
							}
						}
						
						if (teleported == world.getPlayersCount())
						{
							world.setStatus(9);
							world.spawnGroup("boss");
							world.setParameter("TELEPORTED", 0);
						}
						else
						{
							world.setParameter("TELEPORTED", teleported);
						}
						
						startQuestTimer("check_status", 3000, null, player);
						break;
					}
				}
				break;
			}
		}
		
		return null;
	}
	
	@Override
	public void onMoveFinished(Npc npc)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			npc.setInvul(false);
			npc.setTargetable(true);
			
			double minDistance = Double.MAX_VALUE;
			Player target = null;
			for (Player player : world.getPlayers())
			{
				final double distance = player.calculateDistance2D(npc);
				if (distance < minDistance)
				{
					target = player;
					minDistance = distance;
				}
			}
			
			if (target != null)
			{
				npc.setRunning();
				npc.getAI().setIntention(Intention.ATTACK, target);
				npc.asAttackable().addDamageHate(target, 1, 999);
			}
		}
		
		super.onMoveFinished(npc);
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isPet)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			switch (npc.getId())
			{
				case BURNSTEIN:
				{
					world.finishInstance();
					showOnScreenMsg(world, NpcStringId.YOU_VE_SUCCESSFULLY_ATTACKED_THE_COMMAND_POST_AND_DEFEATED_COMMANDER_BURNSTEIN, ExShowScreenMessage.TOP_CENTER, 10000, true);
					break;
				}
				case ADOLPH:
				{
					final Npc adolph = world.spawnGroup("adolph").stream().findFirst().get();
					adolph.setInvul(true);
					adolph.setRandomWalking(false);
					adolph.setTargetable(false);
					addMoveToDesire(adolph, ADOLPH_MOVE, 6);
					break;
				}
				case BARTON:
				{
					final Npc barton = world.spawnGroup("barton").stream().findFirst().get();
					barton.setInvul(true);
					barton.setRandomWalking(false);
					barton.setTargetable(false);
					addMoveToDesire(barton, BARTON_MOVE, 6);
					break;
				}
				case HAYUK:
				{
					final Npc hayuk = world.spawnGroup("hayuk").stream().findFirst().get();
					hayuk.setInvul(true);
					hayuk.setRandomWalking(false);
					hayuk.setTargetable(false);
					addMoveToDesire(hayuk, HAYUK_MOVE, 6);
					break;
				}
				case ELISE:
				{
					final Npc elise = world.spawnGroup("elise").stream().findFirst().get();
					elise.setInvul(true);
					elise.setRandomWalking(false);
					elise.setTargetable(false);
					addMoveToDesire(elise, ELISE_MOVE, 6);
					break;
				}
				case ELRYAH:
				{
					final Npc elryah = world.spawnGroup("elryah").stream().findFirst().get();
					elryah.setInvul(true);
					elryah.setRandomWalking(false);
					elryah.setTargetable(false);
					addMoveToDesire(elryah, ELRYAH_MOVE, 6);
					break;
				}
			}
			
			startQuestTimer("check_status", 3000, null, player);
		}
	}
	
	public static void main(String[] args)
	{
		new CommandPost();
	}
}
