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
package instances.AshenShadowRevolutionaries;

import java.util.List;

import org.l2jmobius.gameserver.managers.InstanceManager;
import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.skill.AbnormalVisualEffect;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.model.zone.type.ScriptZone;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * @author Mobius, Liamxroy
 * @URL https://l2wiki.com/Ashen_Shadow_Revolutionaries
 * @VIDEO https://www.youtube.com/watch?v=ohkxylKJAtQ
 */
public class AshenShadowRevolutionaries extends InstanceScript
{
	// NPCs
	private static final int NETI = 34095;
	private static final int TREASURE_CHEST = 34101;
	private static final int[] QUEST_GIVERS =
	{
		34096,
		34097,
		34098,
		34099,
		34100
	};
	
	// Monsters
	private static final int SPY_DWARF = 23650;
	private static final int SIGNALMAN = 23651;
	private static final int[] COMMANDERS =
	{
		23653, // Unit Commander 1
		23654, // Unit Commander 2
		23655, // Unit Commander 2
		23656, // Unit Commander 2
		23657, // Unit Commander 3
		23658, // Unit Commander 4
		23659, // Unit Commander 4
		23660, // Unit Commander 5
		23661, // Unit Commander 6
		23662, // Unit Commander 7
		23663, // Unit Commander 8
		23664, // Unit Commander 8
	};
	private static final int[] REVOLUTIONARIES =
	{
		23616, // Unit 1 Elite Soldier
		23617, // Unit 2 Elite Soldier
		23618, // Unit 3 Elite Soldier
		23619, // Unit 4 Elite Soldier
		23620, // Unit 5 Elite Soldier
		23621, // Unit 6 Elite Soldier
		23622, // Unit 7 Elite Soldier
		23623, // Unit 8 Elite Soldier
		23624, // Unit 1 Elite Soldier
		23625, // Unit 2 Elite Soldier
		23626, // Unit 3 Elite Soldier
		23627, // Unit 4 Elite Soldier
		23628, // Unit 5 Elite Soldier
		23629, // Unit 6 Elite Soldier
		23630, // Unit 7 Elite Soldier
		23631, // Unit 8 Elite Soldier
		23632, // Unit 1 Elite Soldier
		23633, // Unit 2 Elite Soldier
		23634, // Unit 3 Elite Soldier
		23635, // Unit 4 Elite Soldier
		23636, // Unit 5 Elite Soldier
		23637, // Unit 6 Elite Soldier
		23638, // Unit 7 Elite Soldier
		23639, // Unit 8 Elite Soldier
		23640, // Unit 1 Elite Soldier
		23641, // Unit 2 Elite Soldier
		23642, // Unit 3 Elite Soldier
		23643, // Unit 4 Elite Soldier
		23644, // Unit 5 Elite Soldier
		23645, // Unit 6 Elite Soldier
		23646, // Unit 7 Elite Soldier
		23647, // Unit 8 Elite Soldier
		23648, // Dark Crusader (summon)
		23649, // Banshee Queen (summon)
		SIGNALMAN, // Unit Signalman
		23652, // Unit Guard
		34103, // Revolutionaries Altar
	};
	
	// Locations
	private static final Location QUEST_GIVER_LOCATION = new Location(-77648, 155665, -3190, 21220);
	private static final Location COMMANDER_LOCATION_1 = new Location(-81911, 154244, -3177);
	private static final Location COMMANDER_LOCATION_2 = new Location(-83028, 150866, -3128);
	private static final Location[] SPY_DWARF_LOCATION =
	{
		new Location(-81313, 152102, -3124, 21220), // Magic Shop
		new Location(-83168, 155408, -3175, 64238), // Blacksmith Shop
		new Location(-80000, 153379, -3160, 55621), // Grocery Store
	};
	
	// Misc
	private static final NpcStringId[] DWARF_SPY_TEXT =
	{
		NpcStringId.HOW_DID_YOU_KNOW_I_WAS_HERE,
		NpcStringId.WHY_ARE_YOU_SO_LATE_HUH_YOU_ARE_NOT_PART_OF_THE_ASHEN_SHADOW_REVOLUTIONARIES,
		NpcStringId.I_LL_HAVE_TO_SILENCE_YOU_IN_ORDER_TO_HIDE_THE_FACT_I_M_A_SPY,
		NpcStringId.YOU_THINK_YOU_CAN_LEAVE_THIS_PLACE_ALIVE_AFTER_SEEING_ME,
		NpcStringId.WAIT_WAIT_IT_WILL_BE_BETTER_FOR_YOU_IF_YOU_LET_ME_LIVE,
		NpcStringId.STOP_I_ONLY_HELPED_THE_ASHEN_SHADOW_REVOLUTIONARIES_FOR_A_LITTLE,
	};
	private static final ScriptZone TOWN_ZONE = ZoneManager.getInstance().getZoneById(60200, ScriptZone.class);
	private static final int TEMPLATE_ID = 260;
	
	public AshenShadowRevolutionaries()
	{
		super(TEMPLATE_ID);
		addStartNpc(NETI, TREASURE_CHEST);
		addFirstTalkId(TREASURE_CHEST, 34151, 34152, 34153, 34154, 34155);
		addFirstTalkId(QUEST_GIVERS);
		addTalkId(NETI, TREASURE_CHEST);
		addSpawnId(REVOLUTIONARIES);
		addSpawnId(SPY_DWARF);
		addSpawnId(COMMANDERS);
		addAttackId(SPY_DWARF);
		addKillId(SIGNALMAN);
		addKillId(COMMANDERS);
		addExitZoneId(TOWN_ZONE.getId());
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "enterInstance":
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
				
				if (player.getInstanceWorld() != null)
				{
					startQuestTimer("chest_talk", 1000, player.getInstanceWorld().getNpc(TREASURE_CHEST), null);
				}
				
				return null;
			}
			case "chest_talk":
			{
				final Instance world = npc.getInstanceWorld();
				if ((world != null) && world.isStatus(0))
				{
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.OPEN_THIS_BOX);
					startQuestTimer("chest_talk", 10000, npc, null);
				}
				
				return null;
			}
			case "openBox":
			{
				final Instance world = npc.getInstanceWorld();
				if ((world != null) && world.isStatus(0))
				{
					world.setStatus(1);
					world.spawnGroup("wave_1");
					final Npc questGiver = addSpawn(getRandomEntry(QUEST_GIVERS), QUEST_GIVER_LOCATION, false, 0, false, world.getId());
					questGiver.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THERE_S_NO_ONE_RIGHT);
					if (questGiver.getId() == 34098) // Blacksmith Kluto
					{
						world.spawnGroup("goods");
					}
					
					if (questGiver.getId() == 34100) // Yuyuria
					{
						world.spawnGroup("altars");
					}
					
					if (questGiver.getId() == 34097) // Adonius
					{
						world.setParameter("CAPTIVES", world.spawnGroup("captives"));
						for (Npc captive : world.getParameters().getList("CAPTIVES", Npc.class))
						{
							captive.getEffectList().startAbnormalVisualEffect(AbnormalVisualEffect.FLESH_STONE);
							captive.setTargetable(false);
							captive.broadcastInfo();
						}
					}
					else if (getRandom(10) < 3)
					{
						addSpawn(SPY_DWARF, getRandomEntry(SPY_DWARF_LOCATION), false, 0, false, world.getId());
					}
					
					showOnScreenMsg(world, NpcStringId.ASHEN_SHADOW_REVOLUTIONARIES_KEEP_THE_FORMATION, ExShowScreenMessage.TOP_CENTER, 10000, false);
				}
				
				return null;
			}
			case "exitInstance":
			{
				final Instance world = npc.getInstanceWorld();
				if (world != null)
				{
					world.ejectPlayer(player);
				}
				
				return null;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final Instance world = npc.getInstanceWorld();
		if (world == null)
		{
			return null;
		}
		
		if ((npc.getId() == TREASURE_CHEST) && (world.getStatus() > 0))
		{
			return "34101-1.html";
		}
		
		return npc.getId() + ".html";
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (world == null)
		{
			return;
		}
		
		final int id = npc.getId();
		if (id == SIGNALMAN)
		{
			addSpawn(getRandomEntry(COMMANDERS), world.isStatus(1) ? COMMANDER_LOCATION_1 : COMMANDER_LOCATION_2, false, 0, false, world.getId());
		}
		else if (ArrayUtil.contains(COMMANDERS, id))
		{
			world.incStatus();
			if (world.getStatus() < 3)
			{
				world.spawnGroup("wave_2");
			}
			else
			{
				final List<Npc> captives = world.getParameters().getList("CAPTIVES", Npc.class);
				if (captives != null)
				{
					for (Npc captive : captives)
					{
						captive.setTargetable(true);
						captive.getEffectList().stopAbnormalVisualEffect(AbnormalVisualEffect.FLESH_STONE);
						captive.getEffectList().startAbnormalVisualEffect(AbnormalVisualEffect.MAGIC_SQUARE);
						captive.broadcastInfo();
					}
				}
				
				world.spawnGroup("wave_3");
				world.finishInstance();
			}
		}
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if (getRandom(10) < 1)
		{
			npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(DWARF_SPY_TEXT));
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.setRandomWalking(false);
		if (npc.getId() == 34103)
		{
			npc.setImmobilized(true);
			npc.detachAI();
		}
	}
	
	@Override
	public void onExitZone(Creature creature, ZoneType zone)
	{
		final Instance world = creature.getInstanceWorld();
		if (creature.isPlayer() && (world != null))
		{
			creature.asPlayer().teleToLocation(world.getEnterLocation());
		}
	}
	
	public static void main(String[] args)
	{
		new AshenShadowRevolutionaries();
	}
}
