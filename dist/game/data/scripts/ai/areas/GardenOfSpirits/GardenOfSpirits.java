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
package ai.areas.GardenOfSpirits;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.zone.ZoneType;

/**
 * Garden of Spirits teleport zones.
 * @author Mobius
 */
public class GardenOfSpirits extends Script
{
	// Zones
	private static final int ZONE_ID_1 = 200213;
	private static final int ZONE_ID_2 = 200214;
	private static final int ZONE_ID_3 = 200215;
	private static final int ZONE_ID_4 = 200216;
	private static final int ZONE_ID_5 = 200217;
	private static final int ZONE_ID_6 = 200218;
	private static final int ZONE_ID_7 = 200219;
	private static final int ZONE_ID_8 = 200220;
	
	// Teleport Locations
	private static final Location TELEPORT_LOC_1 = new Location(-55613, 84681, -4560);
	private static final Location TELEPORT_LOC_2 = new Location(-53634, 88711, -3944);
	private static final Location TELEPORT_LOC_3 = new Location(-42123, 79254, -4056);
	private static final Location TELEPORT_LOC_4 = new Location(-45887, 77906, -3656);
	private static final Location TELEPORT_LOC_5 = new Location(-34900, 83422, -3512);
	private static final Location TELEPORT_LOC_6 = new Location(-42123, 79254, -4056);
	private static final Location TELEPORT_LOC_7 = new Location(-42123, 91991, -3793);
	private static final Location TELEPORT_LOC_8 = new Location(-39725, 80900, -3931);
	
	private GardenOfSpirits()
	{
		addEnterZoneId(ZONE_ID_1, ZONE_ID_2, ZONE_ID_3, ZONE_ID_4, ZONE_ID_5, ZONE_ID_6, ZONE_ID_7, ZONE_ID_8);
	}
	
	@Override
	public void onEnterZone(Creature creature, ZoneType zone)
	{
		if (creature.isPlayer())
		{
			switch (zone.getId())
			{
				case ZONE_ID_1:
				{
					creature.teleToLocation(TELEPORT_LOC_1);
					break;
				}
				case ZONE_ID_2:
				{
					creature.teleToLocation(TELEPORT_LOC_2);
					break;
				}
				case ZONE_ID_3:
				{
					creature.teleToLocation(TELEPORT_LOC_3);
					break;
				}
				case ZONE_ID_4:
				{
					creature.teleToLocation(TELEPORT_LOC_4);
					break;
				}
				case ZONE_ID_5:
				{
					creature.teleToLocation(TELEPORT_LOC_5);
					break;
				}
				case ZONE_ID_6:
				{
					creature.teleToLocation(TELEPORT_LOC_6);
					break;
				}
				case ZONE_ID_7:
				{
					creature.teleToLocation(TELEPORT_LOC_7);
					break;
				}
				case ZONE_ID_8:
				{
					creature.teleToLocation(TELEPORT_LOC_8);
					break;
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new GardenOfSpirits();
	}
}
