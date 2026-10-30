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
package ai.areas.HellboundIsland.Masa;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

/**
 * @author Notorion
 */
public class Masa extends Script
{
	// NPC
	private static final int MASA_NPC_ID = 34257;
	
	// Misc
	private static final int DETECTION_RADIUS = 450;
	private static final Set<Player> SEEN_PLAYERS = new HashSet<>();
	
	private Masa()
	{
		addCreatureSeeId(MASA_NPC_ID);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (npc != null)
		{
			if (event.equals("FIRST_MESSAGE"))
			{
				npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.PLEASE_RETURN_THE_EXPERIMENT_SUBJECTS_TO_THEIR_ORIGINAL_FORM);
				startQuestTimer("SECOND_MESSAGE", 5000, npc, player);
				startQuestTimer("FORGET_PLAYER", 10000, npc, player);
			}
			else if (event.equals("SECOND_MESSAGE"))
			{
				npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_MAY_FALL_INTO_DANGER_WITH_A_SUDDEN_ATTACK_SO_BE_CAREFUL);
			}
			else if (event.equals("FORGET_PLAYER") && (player != null))
			{
				SEEN_PLAYERS.remove(player);
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer())
		{
			final Player player = creature.asPlayer();
			if (!SEEN_PLAYERS.contains(player))
			{
				final double distance = npc.calculateDistance3D(player);
				if (distance <= DETECTION_RADIUS)
				{
					SEEN_PLAYERS.add(player);
					startQuestTimer("FIRST_MESSAGE", 0, npc, player);
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new Masa();
	}
}
