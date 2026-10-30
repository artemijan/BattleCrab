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
package ai.areas.AncientCityArcan;

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

import org.l2jmobius.gameserver.data.xml.SpawnData;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.spawns.SpawnGroup;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.model.zone.type.ScriptZone;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.Earthquake;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.OnEventTrigger;

import instances.TaintedDimension.TaintedDimension;
import quests.Q10301_ShadowOfTerrorBlackishRedFog.Q10301_ShadowOfTerrorBlackishRedFog;

/**
 * Ancient Arcan City AI.
 * @author St3eT, Mobius
 */
public class AncientArcanCity extends Script
{
	// NPC
	private static final int CEREMONIAL_CAT = 33093;
	
	// Location
	private static final Location ANCIENT_ARCAN_CITY = new Location(207559, 86429, -1000);
	private static final Location EARTHQUAKE = new Location(207088, 88720, -1128);
	
	// Zones
	private static final ScriptZone BROADCAST_ZONE = ZoneManager.getInstance().getZoneById(23600, ScriptZone.class); // Ancient Arcan City zone
	private static final ScriptZone TELEPORT_ZONE = ZoneManager.getInstance().getZoneById(12015, ScriptZone.class); // Anghel Waterfall teleport zone
	
	// Misc
	private static final SpawnGroup CEREMONY_SPAWNS = SpawnData.getInstance().getSpawnGroupByName("ArcanCeremony");
	private static final Set<Npc> CEREMONIAL_CATS = ConcurrentHashMap.newKeySet();
	private static final int CHANGE_STATE_TIME = 1800000; // 30min
	private static boolean _isCeremonyRunning = false;
	
	private AncientArcanCity()
	{
		addEnterZoneId(TELEPORT_ZONE.getId());
		if (CEREMONY_SPAWNS != null)
		{
			addSpawnId(CEREMONIAL_CAT);
			addEnterZoneId(BROADCAST_ZONE.getId());
			startQuestTimer("CHANGE_STATE", CHANGE_STATE_TIME, null, null, true);
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("CHANGE_STATE"))
		{
			_isCeremonyRunning = !_isCeremonyRunning;
			for (Player plr : BROADCAST_ZONE.getPlayersInside())
			{
				plr.sendPacket(new OnEventTrigger(262001, !_isCeremonyRunning));
				plr.sendPacket(new OnEventTrigger(262003, _isCeremonyRunning));
				if (_isCeremonyRunning)
				{
					showOnScreenMsg(plr, NpcStringId.THE_INCREASED_GRASP_OF_DARK_ENERGY_CAUSES_THE_GROUND_TO_SHAKE, ExShowScreenMessage.TOP_CENTER, 5000, true);
					plr.sendPacket(new Earthquake(EARTHQUAKE, 10, 5));
				}
			}
			
			if (_isCeremonyRunning)
			{
				CEREMONY_SPAWNS.spawnAll();
			}
			else
			{
				cancelQuestTimers("SOCIAL_ACTION");
				CEREMONY_SPAWNS.despawnAll();
				CEREMONIAL_CATS.clear();
			}
		}
		else if (event.equals("SOCIAL_ACTION"))
		{
			for (Npc cat : CEREMONIAL_CATS)
			{
				cat.broadcastSocialAction(2);
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onEnterZone(Creature creature, ZoneType zone)
	{
		if (creature.isPlayer())
		{
			final Player player = creature.asPlayer();
			if (zone.getId() == TELEPORT_ZONE.getId())
			{
				final QuestState qs = player.getQuestState(Q10301_ShadowOfTerrorBlackishRedFog.class.getSimpleName());
				if ((qs != null) && qs.isCond(3))
				{
					final Quest instance = ScriptManager.getInstance().getScript(TaintedDimension.class.getSimpleName());
					if (instance != null)
					{
						instance.notifyEvent("enterInstance", null, player);
					}
				}
				else
				{
					player.teleToLocation(ANCIENT_ARCAN_CITY);
				}
			}
			else
			{
				player.sendPacket(new OnEventTrigger(262001, !_isCeremonyRunning));
				player.sendPacket(new OnEventTrigger(262003, _isCeremonyRunning));
				if (player.getVariables().getBoolean("ANCIENT_ARCAN_CITY_SCENE", true))
				{
					player.getVariables().set("ANCIENT_ARCAN_CITY_SCENE", false);
					playMovie(player, Movie.SI_ARKAN_ENTER);
				}
			}
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		CEREMONIAL_CATS.add(npc);
		npc.setRandomAnimation(false);
		startQuestTimer("SOCIAL_ACTION", 4500, null, null, true);
	}
	
	public static void main(String[] args)
	{
		new AncientArcanCity();
	}
}
