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
package instances.AdventOfDelusion;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;

import quests.Q10376_BloodyGoodTime.Q10376_BloodyGoodTime;

/**
 * Advent Of Delusion instance zone.
 * @author Trevor The Third
 */
public class AdventOfDelusion extends InstanceScript
{
	// Boss
	private static final int BLOOD_THIRST = 27481;
	
	// Misc
	private static final int TEMPLATE_ID = 211;
	
	// Location
	private static final Location GODDARD = new Location(147725, -56517, -2780);
	
	public AdventOfDelusion()
	{
		super(TEMPLATE_ID);
		addKillId(BLOOD_THIRST);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = player.getQuestState(Q10376_BloodyGoodTime.class.getSimpleName());
		if (event.equals("enterInstance"))
		{
			enterInstance(player, npc, TEMPLATE_ID);
			qs.setCond(3);
		}
		else if (event.equals("exitInstance"))
		{
			final Instance world = getPlayerInstance(player);
			if (world != null)
			{
				player.teleToLocation(GODDARD);
				player.setInstance(null);
				qs.setCond(5);
			}
			else
			{
				player.teleToLocation(GODDARD);
				qs.setCond(5);
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			final QuestState qs = player.getQuestState(Q10376_BloodyGoodTime.class.getSimpleName());
			if ((qs != null) && qs.isCond(3))
			{
				qs.setCond(4, true);
			}
		}
	}
	
	public static void main(String[] args)
	{
		new AdventOfDelusion();
	}
}
