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
package ai.areas.GardenOfGenesis.Apherus;

import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.player.TeleportWhereType;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * Apherus RB
 * @author Gigi
 */
public class Apherus extends Script
{
	private static final int APHERUS = 25775;
	private static final int APHERUS_SUBORDINATE = 25865;
	private static final int[] APHERUS_DOOR_NPCS =
	{
		33133,
		33134,
		33135,
		33136
	};
	private static int[] APHERUS_DOORS =
	{
		26210041,
		26210042,
		26210043,
		26210044
	};
	private static final int[] APHERUS_DOOR_GUARD =
	{
		25776,
		25777,
		25778
	};
	private static final SkillHolder GARDEN_APHERUS_RECOVERY = new SkillHolder(14088, 1);
	private static final SkillHolder APHERUS_INVICIBILITY = new SkillHolder(14201, 1);
	private static final int APERUS_KEY = 17373;
	private static final int APHERUS_ZONE_ID = 60060;
	private static boolean _doorIsOpen = false;
	
	public Apherus()
	{
		addAttackId(APHERUS);
		addKillId(APHERUS);
		addSpawnId(APHERUS);
		addTalkId(APHERUS_DOOR_NPCS);
		addStartNpc(APHERUS_DOOR_NPCS);
		addFirstTalkId(APHERUS_DOOR_NPCS);
		addEnterZoneId(APHERUS_ZONE_ID);
		addExitZoneId(APHERUS_ZONE_ID);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("buff"))
		{
			World.getInstance().forEachVisibleObjectInRange(npc, Npc.class, 3000, apherus ->
			{
				if ((apherus.getId() == APHERUS))
				{
					apherus.stopSkillEffects(APHERUS_INVICIBILITY.getSkill());
				}
			});
			
			return null;
		}
		
		return event;
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if ((getRandom(120) < 2) && !npc.isDead())
		{
			final Npc minions = addSpawn(APHERUS_SUBORDINATE, npc.getX() + getRandom(-30, 30), npc.getY() + getRandom(-30, 30), npc.getZ(), npc.getHeading(), true, 300000, false);
			addAttackPlayerDesire(minions, attacker);
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		World.getInstance().forEachVisibleObjectInRange(npc, Npc.class, 1500, minion ->
		{
			if (minion.getId() == APHERUS_SUBORDINATE)
			{
				minion.deleteMe();
			}
		});
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		APHERUS_INVICIBILITY.getSkill().applyEffects(npc, npc);
		_doorIsOpen = false;
		for (int door : APHERUS_DOORS)
		{
			closeDoor(door, npc.getInstanceId());
		}
	}
	
	@Override
	public void onEnterZone(Creature creature, ZoneType zone)
	{
		if (creature.isRaid())
		{
			creature.stopSkillEffects(GARDEN_APHERUS_RECOVERY.getSkill());
		}
		else if (creature.isPlayable() && !_doorIsOpen && !creature.isGM())
		{
			creature.teleToLocation(TeleportWhereType.TOWN);
		}
	}
	
	@Override
	public void onExitZone(Creature creature, ZoneType zone)
	{
		if (creature.isRaid())
		{
			GARDEN_APHERUS_RECOVERY.getSkill().applyEffects(creature, creature);
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		if (!_doorIsOpen)
		{
			if (!player.destroyItemByItemId(ItemProcessType.FEE, APERUS_KEY, 1, player, true))
			{
				return "apherusDoor-no.html";
			}
			
			if (getRandom(100) > 60)
			{
				startQuestTimer("buff", 500, npc, player);
				for (int door : APHERUS_DOORS)
				{
					openDoor(door, npc.getInstanceId());
				}
				
				npc.broadcastPacket(new ExShowScreenMessage(NpcStringId.APHERUS_GARDEN_DOOR_WAS_OPENED, ExShowScreenMessage.TOP_CENTER, 3000, true));
			}
			else
			{
				for (int i = 0; i < 4; i++)
				{
					final Monster protector = addSpawn(APHERUS_DOOR_GUARD[getRandom(APHERUS_DOOR_GUARD.length)], player.getX() + getRandom(10, 30), player.getY() + getRandom(10, 30), player.getZ(), 0, false, 600000, false).asMonster();
					protector.setRunning();
					protector.setTarget(player);
					addAttackPlayerDesire(protector, player);
				}
				
				showOnScreenMsg(player, NpcStringId.S1_THE_KEY_DOES_NOT_MATCH_SO_WE_RE_IN_TROUBLE, ExShowScreenMessage.TOP_CENTER, 6000, true, player.getName());
			}
		}
		else
		{
			return "apherusDoor-no.html";
		}
		
		return null;
	}
	
	public static void main(String[] args)
	{
		new Apherus();
	}
}
