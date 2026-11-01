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
package ai.areas.Parnassus.EntrancePortalToCrystalCaverns;

import java.util.Calendar;

import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.serverpackets.OnEventTrigger;

import instances.CrystalCaverns.CrystalCavernsCoralGarden;
import instances.CrystalCaverns.CrystalCavernsEmeraldSquare;
import instances.CrystalCaverns.CrystalCavernsSteamCorridor;

/**
 * Entrance Portal to Crystal Caverns AI.
 * @author St3eT
 */
public class EntrancePortalToCrystalCaverns extends Script
{
	// NPCs
	private static final int CAVERNS_ENTRACE = 33522;
	
	// Misc
	private static final int EMERALD_SQUARE_TEMPLATE_ID = 163;
	private static final int STEAM_CORRIDOR_TEMPLATE_ID = 164;
	private static final int CORAL_GARDEN_TEMPLATE_ID = 165;
	private static final int PRISON_ENTRACE_TRIGGER_1 = 24230010;
	private static final int PRISON_ENTRACE_TRIGGER_2 = 24230012;
	private static final int CAVERNS_ENTRACE_TRIGGER_1 = 24230014;
	private static final int CAVERNS_ENTRACE_TRIGGER_2 = 24230016;
	private static final int CAVERNS_ENTRACE_TRIGGER_3 = 24230018;
	
	private EntrancePortalToCrystalCaverns()
	{
		addStartNpc(CAVERNS_ENTRACE);
		addTalkId(CAVERNS_ENTRACE);
		addFirstTalkId(CAVERNS_ENTRACE);
		addSpawnId(CAVERNS_ENTRACE);
		addCreatureSeeId(CAVERNS_ENTRACE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("enterInstance"))
		{
			Quest instanceScript = null;
			
			switch (getCurrentInstanceTemplateId())
			{
				case EMERALD_SQUARE_TEMPLATE_ID:
				{
					instanceScript = ScriptManager.getInstance().getScript(CrystalCavernsEmeraldSquare.class.getSimpleName());
					break;
				}
				case STEAM_CORRIDOR_TEMPLATE_ID:
				{
					instanceScript = ScriptManager.getInstance().getScript(CrystalCavernsSteamCorridor.class.getSimpleName());
					break;
				}
				case CORAL_GARDEN_TEMPLATE_ID:
				{
					instanceScript = ScriptManager.getInstance().getScript(CrystalCavernsCoralGarden.class.getSimpleName());
					break;
				}
			}
			
			if (instanceScript != null)
			{
				instanceScript.notifyEvent(event, npc, player);
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "EntrancePortal_" + getCurrentInstanceTemplateId() + ".html";
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.setDisplayEffect(1);
		getTimers().addRepeatingTimer("LOOP_TIMER", 10000, npc, null);
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		if (event.equals("LOOP_TIMER"))
		{
			final int currentTemplateId = getCurrentInstanceTemplateId();
			World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 500, p -> updateTriggersForPlayer(player, currentTemplateId));
		}
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer())
		{
			final Player player = creature.asPlayer();
			player.sendPacket(new OnEventTrigger(PRISON_ENTRACE_TRIGGER_1, true));
			player.sendPacket(new OnEventTrigger(PRISON_ENTRACE_TRIGGER_2, true));
			updateTriggersForPlayer(player, getCurrentInstanceTemplateId());
		}
	}
	
	public void updateTriggersForPlayer(Player player, int currentTemplateId)
	{
		if (player != null)
		{
			player.sendPacket(new OnEventTrigger(CAVERNS_ENTRACE_TRIGGER_1, false));
			player.sendPacket(new OnEventTrigger(CAVERNS_ENTRACE_TRIGGER_2, false));
			player.sendPacket(new OnEventTrigger(CAVERNS_ENTRACE_TRIGGER_3, false));
			
			switch (currentTemplateId)
			{
				case EMERALD_SQUARE_TEMPLATE_ID:
				{
					player.sendPacket(new OnEventTrigger(CAVERNS_ENTRACE_TRIGGER_1, true));
					break;
				}
				case STEAM_CORRIDOR_TEMPLATE_ID:
				{
					player.sendPacket(new OnEventTrigger(CAVERNS_ENTRACE_TRIGGER_2, true));
					break;
				}
				case CORAL_GARDEN_TEMPLATE_ID:
				{
					player.sendPacket(new OnEventTrigger(CAVERNS_ENTRACE_TRIGGER_3, true));
					break;
				}
			}
		}
	}
	
	public int getCurrentInstanceTemplateId()
	{
		final int day = Calendar.getInstance().get(Calendar.DAY_OF_WEEK);
		final int hour = Calendar.getInstance().get(Calendar.HOUR_OF_DAY);
		int templateId = -1;
		
		switch (day)
		{
			case Calendar.MONDAY:
			{
				templateId = (hour < 18) ? EMERALD_SQUARE_TEMPLATE_ID : STEAM_CORRIDOR_TEMPLATE_ID;
				break;
			}
			case Calendar.TUESDAY:
			{
				templateId = (hour < 18) ? CORAL_GARDEN_TEMPLATE_ID : EMERALD_SQUARE_TEMPLATE_ID;
				break;
			}
			case Calendar.WEDNESDAY:
			{
				templateId = (hour < 18) ? STEAM_CORRIDOR_TEMPLATE_ID : CORAL_GARDEN_TEMPLATE_ID;
				break;
			}
			case Calendar.THURSDAY:
			{
				templateId = (hour < 18) ? EMERALD_SQUARE_TEMPLATE_ID : STEAM_CORRIDOR_TEMPLATE_ID;
				break;
			}
			case Calendar.FRIDAY:
			{
				templateId = (hour < 18) ? CORAL_GARDEN_TEMPLATE_ID : EMERALD_SQUARE_TEMPLATE_ID;
				break;
			}
			case Calendar.SATURDAY:
			{
				templateId = (hour < 18) ? STEAM_CORRIDOR_TEMPLATE_ID : CORAL_GARDEN_TEMPLATE_ID;
				break;
			}
			case Calendar.SUNDAY:
			{
				templateId = (hour < 18) ? EMERALD_SQUARE_TEMPLATE_ID : STEAM_CORRIDOR_TEMPLATE_ID;
				break;
			}
		}
		
		return templateId;
	}
	
	public static void main(String[] args)
	{
		new EntrancePortalToCrystalCaverns();
	}
}
