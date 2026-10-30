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
package ai.areas.TalkingIsland.HarnakUndergroundRuinsZone;

import java.util.HashMap;
import java.util.Map;
import java.util.Map.Entry;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.spawns.SpawnTemplate;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExSendUIEvent;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * Harnak Underground Ruins
 * @video https://www.youtube.com/watch?v=0IdcAB3aYO0
 * @author LasTravel, Gigi
 * @date 2017-03-10 - [13:09:05]
 */
public class HarnakUndergroundRuinsZone extends Script
{
	// @formatter:off
	private static final int[] NORMAL_MOBS = {22931, 22932, 22933, 22934, 22935, 22936, 22937, 22938, 23349};
	// @formatter:on
	static Map<ZoneType, zoneInfo> _roomInfo = new HashMap<>(24);
	static final Set<SpawnTemplate> _templates = ConcurrentHashMap.newKeySet();
	
	public HarnakUndergroundRuinsZone()
	{
		addKillId(NORMAL_MOBS);
		addSpawnId(NORMAL_MOBS);
		startQuestTimer("HARNAK_SPAWN", 15000, null, null, false);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("HARNAK_SPAWN"))
		{
			for (int zoneId = 60142; zoneId <= 60165; zoneId++)
			{
				final ZoneType zone = ZoneManager.getInstance().getZoneById(zoneId);
				_roomInfo.put(zone, new zoneInfo());
				final String zoneName = zone.getName().toLowerCase().replace(" ", "_");
				_templates.forEach(t -> t.spawn(g -> String.valueOf(g.getName()).equalsIgnoreCase(zoneName), null));
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	private static class zoneInfo
	{
		private int currentPoints = 0;
		private int currentMonitorizedDamage = 0;
		private int zoneStage = 0;
		
		public zoneInfo()
		{
		}
		
		void setZoneStage(int a)
		{
			zoneStage = a;
		}
		
		void setCurrentPoint(int a)
		{
			currentPoints = a;
		}
		
		void setMonitorizedDamage(int a)
		{
			currentMonitorizedDamage = a;
		}
		
		int getZoneStage()
		{
			return zoneStage;
		}
		
		int getCurrentPoints()
		{
			return currentPoints;
		}
		
		int getCurrentMonitorizedDamage()
		{
			return currentMonitorizedDamage;
		}
		
		void reset()
		{
			currentPoints = 0;
			currentMonitorizedDamage = 0;
			zoneStage = 0;
		}
	}
	
	private static class changeZoneStage implements Runnable
	{
		private final ZoneType _zone;
		
		public changeZoneStage(ZoneType zone)
		{
			_zone = zone;
		}
		
		@Override
		public void run()
		{
			try
			{
				final zoneInfo currentInfo = _roomInfo.get(_zone);
				switch (currentInfo.getZoneStage())
				{
					case 0:
					{
						_zone.broadcastPacket(new ExShowScreenMessage(NpcStringId.MONITOR_THE_DAMAGE_FOR_30_SEC, ExShowScreenMessage.TOP_CENTER, 3000));
						break;
					}
					case 1:
					{
						_zone.broadcastPacket(new ExShowScreenMessage(NpcStringId.TWENTY_FIVE_SECONDS_LEFT, ExShowScreenMessage.TOP_CENTER, 3000));
						break;
					}
					case 2:
					{
						_zone.broadcastPacket(new ExShowScreenMessage(NpcStringId.TWENTY_SECONDS_LEFT, ExShowScreenMessage.TOP_CENTER, 3000));
						break;
					}
					case 3:
					{
						_zone.broadcastPacket(new ExShowScreenMessage(NpcStringId.FIFTEEN_SECONDS_LEFT, ExShowScreenMessage.TOP_CENTER, 3000));
						break;
					}
					case 4:
					{
						_zone.broadcastPacket(new ExShowScreenMessage(NpcStringId.TEN_SECONDS_LEFT, ExShowScreenMessage.TOP_CENTER, 3000));
						break;
					}
					case 5:
					{
						_zone.broadcastPacket(new ExShowScreenMessage(NpcStringId.FIVE_SECONDS_LEFT, ExShowScreenMessage.TOP_CENTER, 3000));
						break;
					}
					case 6:
					{
						if (currentInfo.getCurrentMonitorizedDamage() >= 10)
						{
							_zone.broadcastPacket(new ExShowScreenMessage(NpcStringId.DEMONIC_SYSTEM_WILL_ACTIVATE, ExShowScreenMessage.TOP_CENTER, 3000));
							final String zoneName = _zone.getName().toLowerCase().replace(" ", "_");
							_templates.forEach(t -> t.despawn(g -> String.valueOf(g.getName()).equalsIgnoreCase(zoneName)));
							_templates.forEach(t -> t.spawn(g -> String.valueOf(g.getName()).equalsIgnoreCase(zoneName + "_demonic"), null));
							_zone.getPlayersInside().forEach(temp -> temp.sendPacket(new ExSendUIEvent(temp, false, false, 600, 0, NpcStringId.DEMONIC_SYSTEM_ACTIVATED)));
							currentInfo.setZoneStage(7);
							ThreadPool.schedule(new changeZoneStage(_zone), 600000); // 10min
						}
						else
						{
							currentInfo.reset();
							return;
						}
						break;
					}
					case 7:
					{
						currentInfo.reset();
						for (Player player : _zone.getPlayersInside())
						{
							if (player != null)
							{
								player.sendPacket(new ExSendUIEvent(player));
							}
						}
						
						final String zoneName = _zone.getName().toLowerCase().replace(" ", "_");
						_templates.forEach(t -> t.despawn(g -> String.valueOf(g.getName()).equalsIgnoreCase(zoneName + "_demonic")));
						_templates.forEach(t -> t.spawn(g -> String.valueOf(g.getName()).equalsIgnoreCase(zoneName), null));
						return;
					}
				}
				
				if (currentInfo.getZoneStage() < 6)
				{
					currentInfo.setZoneStage(currentInfo.getZoneStage() + 1);
					ThreadPool.schedule(new changeZoneStage(_zone), 5000);
				}
			}
			catch (Exception e)
			{
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		for (Entry<ZoneType, zoneInfo> currentZone : _roomInfo.entrySet())
		{
			if (currentZone.getKey().isInsideZone(npc))
			{
				final zoneInfo currentInfo = currentZone.getValue();
				final int currentPoints = currentInfo.getCurrentPoints();
				if (currentPoints == 300)
				{
					if (currentInfo.getZoneStage() < 1)
					{
						return;
					}
					
					final int currentDamage = currentInfo.getCurrentMonitorizedDamage();
					int calcDamage = currentDamage + 1;
					if (calcDamage >= 10)
					{
						calcDamage = 10;
					}
					
					currentInfo.setMonitorizedDamage(calcDamage);
					for (Player player : currentZone.getKey().getPlayersInside())
					{
						if (player != null)
						{
							player.sendPacket(new ExSendUIEvent(player, ExSendUIEvent.TYPE_NORNIL, calcDamage, 10, NpcStringId.MONITOR_THE_DAMAGE));
						}
					}
					return;
				}
				
				int calcPoints = currentPoints + 1;
				if (calcPoints >= 300)
				{
					calcPoints = 300;
					ThreadPool.schedule(new changeZoneStage(currentZone.getKey()), 1000);
				}
				
				currentInfo.setCurrentPoint(calcPoints);
				for (Player player : currentZone.getKey().getPlayersInside())
				{
					if (player != null)
					{
						player.sendPacket(new ExSendUIEvent(player, ExSendUIEvent.TYPE_NORNIL, calcPoints, 300, NpcStringId.DANGER_INCREASING_DANGER_INCREASING));
					}
				}
			}
		}
		
		if (npc.getDisplayEffect() > 0)
		{
			final Monster copy = addSpawn(npc.getId(), npc.getX(), npc.getY(), npc.getZ(), 0, true, 0, false).asMonster();
			copy.setTarget(killer);
			copy.addDamageHate(killer, 500, 99999);
			copy.getAI().setIntention(Intention.ATTACK, killer);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		if (getRandom(20) > 18)
		{
			npc.setDisplayEffect(1);
		}
	}
	
	@Override
	public void onSpawnActivate(SpawnTemplate template)
	{
		_templates.add(template);
	}
	
	public static void main(String[] args)
	{
		new HarnakUndergroundRuinsZone();
	}
}
