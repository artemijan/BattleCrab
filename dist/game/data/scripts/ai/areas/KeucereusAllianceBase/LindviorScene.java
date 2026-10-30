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
package ai.areas.KeucereusAllianceBase;

import java.util.Calendar;
import java.util.GregorianCalendar;

import org.l2jmobius.gameserver.data.SpawnTable;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.spawns.Spawn;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.enums.Movie;

/**
 * Lindvior Scene AI.
 * @author nonom, Mobius
 */
public class LindviorScene extends Script
{
	private static final int LINDVIOR_CAMERA = 18669;
	private static final int TOMARIS = 32552;
	private static final int ARTIUS = 32559;
	
	private static final int RESET_HOUR = 18;
	private static final int RESET_MIN = 58;
	private static final int RESET_DAY_1 = Calendar.TUESDAY;
	private static final int RESET_DAY_2 = Calendar.WEDNESDAY;
	private static final int RESET_DAY_3 = Calendar.THURSDAY;
	private static final int RESET_DAY_4 = Calendar.FRIDAY;
	
	private static boolean ALT_MODE = false;
	private static int ALT_MODE_MIN = 120; // schedule delay in minutes if ALT_MODE enabled
	
	public LindviorScene()
	{
		scheduleNextLindviorVisit();
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "tomaris_shout1":
			{
				npc.broadcastSay(ChatType.NPC_SHOUT, NpcStringId.HUH_THE_SKY_LOOKS_FUNNY_WHAT_S_THAT);
				break;
			}
			case "artius_shout":
			{
				npc.broadcastSay(ChatType.NPC_SHOUT, NpcStringId.A_POWERFUL_SUBORDINATE_IS_BEING_HELD_BY_THE_BARRIER_ORB_THIS_REACTION_MEANS);
				break;
			}
			case "tomaris_shout2":
			{
				npc.broadcastSay(ChatType.NPC_SHOUT, NpcStringId.BE_CAREFUL_SOMETHING_S_COMING);
				break;
			}
			case "lindvior_scene":
			{
				if (npc != null)
				{
					playMovie(World.getInstance().getVisibleObjectsInRange(npc, Player.class, 4000), Movie.SC_LINDVIOR);
				}
				break;
			}
			case "start":
			{
				final Spawn cameraSpawn = SpawnTable.getInstance().getAnySpawn(LINDVIOR_CAMERA);
				final Spawn tomarisSpawn = SpawnTable.getInstance().getAnySpawn(TOMARIS);
				final Spawn artiusSpawn = SpawnTable.getInstance().getAnySpawn(ARTIUS);
				if ((cameraSpawn == null) || (tomarisSpawn == null) || (artiusSpawn == null))
				{
					break;
				}
				
				final Npc camera = cameraSpawn.getLastSpawn();
				final Npc tomaris = tomarisSpawn.getLastSpawn();
				final Npc artius = artiusSpawn.getLastSpawn();
				if ((camera == null) || (tomaris == null) || (artius == null))
				{
					break;
				}
				
				startQuestTimer("tomaris_shout1", 1000, tomaris, null);
				startQuestTimer("artius_shout", 60000, artius, null);
				startQuestTimer("tomaris_shout2", 90000, tomaris, null);
				startQuestTimer("lindvior_scene", 120000, camera, null);
				scheduleNextLindviorVisit();
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	public void scheduleNextLindviorVisit()
	{
		final long delay = (ALT_MODE) ? ALT_MODE_MIN * 120000 : scheduleNextLindviorDate();
		startQuestTimer("start", delay, null, null);
	}
	
	protected long scheduleNextLindviorDate()
	{
		final GregorianCalendar date = new GregorianCalendar();
		date.set(Calendar.MINUTE, RESET_MIN);
		date.set(Calendar.HOUR_OF_DAY, RESET_HOUR);
		if (System.currentTimeMillis() >= date.getTimeInMillis())
		{
			date.add(Calendar.DAY_OF_WEEK, 1);
		}
		
		final int dayOfWeek = date.get(Calendar.DAY_OF_WEEK);
		if (dayOfWeek <= RESET_DAY_1)
		{
			date.add(Calendar.DAY_OF_WEEK, RESET_DAY_1 - dayOfWeek);
		}
		else if (dayOfWeek <= RESET_DAY_2)
		{
			date.add(Calendar.DAY_OF_WEEK, RESET_DAY_2 - dayOfWeek);
		}
		else if (dayOfWeek <= RESET_DAY_3)
		{
			date.add(Calendar.DAY_OF_WEEK, RESET_DAY_3 - dayOfWeek);
		}
		else if (dayOfWeek <= RESET_DAY_4)
		{
			date.add(Calendar.DAY_OF_WEEK, RESET_DAY_4 - dayOfWeek);
		}
		else
		{
			date.add(Calendar.DAY_OF_WEEK, 1 + RESET_DAY_1);
		}
		
		return date.getTimeInMillis() - System.currentTimeMillis();
	}
	
	public static void main(String[] args)
	{
		new LindviorScene();
	}
}
