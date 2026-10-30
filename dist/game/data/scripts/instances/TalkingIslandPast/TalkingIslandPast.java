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
package instances.TalkingIslandPast;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.Q10385_RedThreadOfFate.Q10385_RedThreadOfFate;

/**
 * Talking Island (Past) instance zone.
 * @author Gladicek, Trevor The Third
 */
public class TalkingIslandPast extends InstanceScript
{
	// NPCs
	private static final int DARIN = 33748;
	private static final int ROXXY = 33749;
	private static final int MYSTERIOUS_DARK_KNIGHT = 33751;
	private static final int INVISIBLE_TI_NPC = 18919;
	
	// Misc
	private static final int TEMPLATE_ID = 241;
	
	// Location
	private static final Location TOWN_TELEPORT = new Location(210799, 13426, -3720);
	private static final Location TI_LOC_1 = new Location(210779, 15547, -3732);
	private static final Location TI_LOC_2 = new Location(209267, 14943, -3729);
	private static final Location TI_LOC_3 = new Location(210332, 13156, -3729);
	
	// Zones
	private static final int TALKING_ISLAND_ZONE = 12035;
	
	public TalkingIslandPast()
	{
		super(TEMPLATE_ID);
		addTalkId(DARIN, ROXXY, MYSTERIOUS_DARK_KNIGHT);
		addFirstTalkId(DARIN, ROXXY, MYSTERIOUS_DARK_KNIGHT);
		addExitZoneId(TALKING_ISLAND_ZONE);
		addCreatureSeeId(INVISIBLE_TI_NPC);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("enterInstance"))
		{
			enterInstance(player, npc, TEMPLATE_ID);
		}
		else if (event.equals("exitInstance"))
		{
			final Instance world = getPlayerInstance(player);
			if (world != null)
			{
				teleportPlayerOut(player, world);
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	@Override
	protected void onEnter(Player player, Instance instance, boolean firstEnter)
	{
		final QuestState qs = player.getQuestState(Q10385_RedThreadOfFate.class.getSimpleName());
		if ((qs != null) && qs.isCond(21) && (qs.isMemoState(2)))
		{
			final Npc knight = addSpawn(MYSTERIOUS_DARK_KNIGHT, TI_LOC_3, false, 0, false, instance.getId());
			knight.getAI().startFollow(player);
			knight.setRunning();
			showOnScreenMsg(player, NpcStringId.A_MYSTERIOUS_DARK_KNIGHT_IS_HERE, ExShowScreenMessage.TOP_CENTER, 5000);
			getTimers().addTimer("MSG", null, 5000, null, player, n -> showOnScreenMsg(n.getPlayer(), NpcStringId.TALK_TO_THE_MYSTERIOUS_DARK_KNIGHT, ExShowScreenMessage.TOP_CENTER, 5000));
		}
		
		super.onEnter(player, instance, firstEnter);
	}
	
	@Override
	public void onExitZone(Creature creature, ZoneType zone)
	{
		final Instance instance = creature.getInstanceWorld();
		if ((instance != null) && creature.isPlayer())
		{
			creature.teleToLocation(TOWN_TELEPORT);
		}
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer())
		{
			final Instance instance = creature.getInstanceWorld();
			final Player player = creature.asPlayer();
			final QuestState qs = player.getQuestState(Q10385_RedThreadOfFate.class.getSimpleName());
			if ((instance != null) && (npc.getId() == INVISIBLE_TI_NPC) && (qs != null) && qs.isCond(21) && qs.isMemoState(1))
			{
				final Location loc = npc.isInsideRadius2D(TI_LOC_1, 1000) ? TI_LOC_1 : TI_LOC_2;
				qs.setMemoState(2);
				final Npc knight = addSpawn(MYSTERIOUS_DARK_KNIGHT, loc, false, 0, false, instance.getId());
				knight.getAI().startFollow(player);
				knight.setRunning();
				showOnScreenMsg(player, NpcStringId.A_MYSTERIOUS_DARK_KNIGHT_IS_HERE, ExShowScreenMessage.TOP_CENTER, 5000);
				getTimers().addTimer("MSG", null, 5000, npc, player, n -> showOnScreenMsg(n.getPlayer(), NpcStringId.TALK_TO_THE_MYSTERIOUS_DARK_KNIGHT, ExShowScreenMessage.TOP_CENTER, 5000));
			}
		}
	}
	
	public static void main(String[] args)
	{
		new TalkingIslandPast();
	}
}
