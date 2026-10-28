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
package ai.bosses.Tauti;

import java.util.List;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.OnEventTrigger;

/**
 * Tauti Warzone instance zone.
 * @author Mobius
 */
public class TautiWarzone extends InstanceScript
{
	// NPCs
	private static final int FINARIA = 33675;
	private static final int ZAHAK = 19266;
	private static final int TELEPORTER = 33678;
	private static final int TAUTI_COMMON = 29233;
	private static final int TAUTI_EXTREME = 29234;
	private static final int TAUTI_COMMON_AXE = 29236;
	private static final int TAUTI_EXTREME_AXE = 29237;
	
	// Item
	private static final int KEY_OF_DARKNESS = 34899;
	
	// Teleport
	private static final Location TAUTI_TELEPORT = new Location(-148972, 209879, -10199);
	
	// Misc
	private static final int TEMPLATE_ID = 218;
	private static final int EXTREME_TEMPLATE_ID = 219;
	private static final int DOOR_1 = 15240001;
	private static final int DOOR_2 = 15240002;
	
	public TautiWarzone()
	{
		super(TEMPLATE_ID, EXTREME_TEMPLATE_ID);
		addTalkId(FINARIA, TELEPORTER);
		addFirstTalkId(FINARIA, TELEPORTER);
		addAttackId(TAUTI_COMMON, TAUTI_EXTREME);
		addKillId(ZAHAK, TAUTI_COMMON_AXE, TAUTI_EXTREME_AXE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "enterEasyInstance":
			{
				enterInstance(player, npc, TEMPLATE_ID);
				if (player.getInstanceWorld() != null)
				{
					startQuestTimer("WORLD_STATUS_CHECK", 5000, player.getInstanceWorld().getNpc(FINARIA), null);
				}
				break;
			}
			case "enterExtremeInstance":
			{
				enterInstance(player, npc, EXTREME_TEMPLATE_ID);
				if (player.getInstanceWorld() != null)
				{
					startQuestTimer("WORLD_STATUS_CHECK", 5000, player.getInstanceWorld().getNpc(FINARIA), null);
				}
				break;
			}
			case "useKey":
			{
				final Instance world = npc.getInstanceWorld();
				if (hasAtLeastOneQuestItem(player, KEY_OF_DARKNESS))
				{
					takeItems(player, KEY_OF_DARKNESS, -1);
					world.setStatus(3);
					npc.broadcastPacket(new OnEventTrigger(15235001, true));
					return "33678-3.htm";
				}
				
				return "33678-1.htm";
			}
			case "teleport":
			{
				final Instance world = npc.getInstanceWorld();
				world.setStatus(4);
				world.openCloseDoor(DOOR_2, true);
				for (Player member : world.getPlayers())
				{
					member.teleToLocation(TAUTI_TELEPORT, world);
					if ((world.getAliveNpcCount(TAUTI_EXTREME) == 0) && (world.getAliveNpcCount(TAUTI_EXTREME_AXE) == 0))
					{
						startQuestTimer("PLAY_OPENING_B_MOVIE", 5000, null, member, false);
					}
				}
				
				startQuestTimer("SPAWN_TAUTI", 60000, npc, null, false);
				break;
			}
			case "PLAY_OPENING_B_MOVIE":
			{
				playMovie(player, Movie.SC_TAUTI_OPENING_B);
				break;
			}
			case "SPAWN_TAUTI":
			{
				final Instance world = npc.getInstanceWorld();
				if ((world.getAliveNpcCount(TAUTI_EXTREME) == 0) && (world.getAliveNpcCount(TAUTI_EXTREME_AXE) == 0))
				{
					world.spawnGroup("boss");
				}
				
				world.openCloseDoor(DOOR_1, true);
				break;
			}
			case "SPAWN_AXE":
			{
				final Instance world = player.getInstanceWorld();
				final Npc axe = addSpawn(world.getTemplateId() == EXTREME_TEMPLATE_ID ? TAUTI_EXTREME_AXE : TAUTI_COMMON_AXE, world.getParameters().getObject("AXE_LOCATION", Location.class), false, 0, false, world.getId());
				axe.setRandomWalking(false);
				axe.setImmobilized(true);
				world.getParameters().remove("AXE_LOCATION");
				break;
			}
			case "WORLD_STATUS_CHECK":
			{
				final Instance world = npc.getInstanceWorld();
				if (world == null)
				{
					return null;
				}
				
				if (world.isStatus(0))
				{
					world.setStatus(1);
					world.spawnGroup("room1");
					playMovie(world.getPlayers(), Movie.SC_TAUTI_OPENING);
					for (Player member : world.getPlayers())
					{
						takeItems(member, KEY_OF_DARKNESS, -1);
					}
					
					startQuestTimer("WORLD_STATUS_CHECK", 5000, npc, null);
				}
				else if (world.isStatus(1))
				{
					if (world.getAliveNpcs(Monster.class).isEmpty())
					{
						world.setStatus(2);
						final List<Npc> monsters = world.spawnGroup("room2");
						for (Npc monster : monsters)
						{
							monster.setRandomWalking(false);
						}
					}
					
					startQuestTimer("WORLD_STATUS_CHECK", 5000, npc, null);
				}
				
				return null;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		String htmltext = "";
		switch (npc.getId())
		{
			case FINARIA:
			{
				htmltext = "33675.htm";
				break;
			}
			case TELEPORTER:
			{
				final int worldStatus = npc.getInstanceWorld().getStatus();
				if (worldStatus > 2)
				{
					htmltext = "33678-3.htm";
				}
				else if ((worldStatus == 2) && hasAtLeastOneQuestItem(player, KEY_OF_DARKNESS))
				{
					htmltext = "33678-2.htm";
				}
				else
				{
					htmltext = "33678-1.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (world.isStatus(5) && (npc.getCurrentHp() < (npc.getMaxHp() * 0.15)))
		{
			world.setStatus(6);
			world.setParameter("AXE_LOCATION", npc.getLocation());
			npc.deleteMe();
			startQuestTimer("SPAWN_AXE", 15000, null, attacker, false);
			playMovie(world.getPlayers(), Movie.SC_TAUTI_PHASE);
		}
		
		if (world.isStatus(4))
		{
			world.setStatus(5);
			world.openCloseDoor(DOOR_1, false);
			world.openCloseDoor(DOOR_2, false);
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (npc.getId() == ZAHAK)
		{
			npc.dropItem(killer, KEY_OF_DARKNESS, 1);
		}
		else
		{
			final Instance world = npc.getInstanceWorld();
			playMovie(world.getPlayers(), Movie.SC_TAUTI_ENDING);
			npc.deleteMe();
			world.finishInstance();
		}
	}
	
	@Override
	public void onInstanceCreated(Instance instance, Player player)
	{
		if (player != null)
		{
			showHtmlFile(player, instance.getTemplateId() == EXTREME_TEMPLATE_ID ? "PartyEnterExtreme.html" : "PartyEnterCommon.html");
		}
	}
	
	public static void main(String[] args)
	{
		new TautiWarzone();
	}
}
