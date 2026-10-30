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
package vehicles.KeucereusNorthController;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.holders.creature.VehiclePathPoint;
import org.l2jmobius.gameserver.network.enums.Movie;

import vehicles.AirShipController;

public class KeucereusNorthController extends AirShipController
{
	private static final int DOCK_ZONE = 50602;
	private static final int LOCATION = 100;
	private static final int CONTROLLER_ID = 32606;
	
	private static final VehiclePathPoint[] ARRIVAL =
	{
		new VehiclePathPoint(-183218, 239494, 2500, 280, 2000),
		new VehiclePathPoint(-183218, 239494, 1336, 280, 2000)
	};
	
	private static final VehiclePathPoint[] DEPART =
	{
		new VehiclePathPoint(-183218, 239494, 1700, 280, 2000),
		new VehiclePathPoint(-181974, 235358, 1700, 280, 2000)
	};
	
	private static final VehiclePathPoint[][] TELEPORTS =
	{
		{
			new VehiclePathPoint(-183218, 239494, 1700, 280, 2000),
			new VehiclePathPoint(-181974, 235358, 1700, 280, 2000),
			new VehiclePathPoint(-186373, 234000, 2500, 0, 0)
		},
		{
			new VehiclePathPoint(-183218, 239494, 1700, 280, 2000),
			new VehiclePathPoint(-181974, 235358, 1700, 280, 2000),
			new VehiclePathPoint(-206692, 220997, 3000, 0, 0)
		},
		{
			new VehiclePathPoint(-183218, 239494, 1700, 280, 2000),
			new VehiclePathPoint(-181974, 235358, 1700, 280, 2000),
			new VehiclePathPoint(-235693, 248843, 5100, 0, 0)
		}
	};
	
	private static final int[] FUEL =
	{
		0,
		50,
		100
	};
	
	public KeucereusNorthController()
	{
		addStartNpc(CONTROLLER_ID);
		addFirstTalkId(CONTROLLER_ID);
		addTalkId(CONTROLLER_ID);
		
		_dockZone = DOCK_ZONE;
		addEnterZoneId(DOCK_ZONE);
		addExitZoneId(DOCK_ZONE);
		
		_shipSpawnX = -184145;
		_shipSpawnY = 242373;
		_shipSpawnZ = 3000;
		_oustLoc = new Location(-183900, 239384, 1320);
		_locationId = LOCATION;
		_arrivalPath = ARRIVAL;
		_departPath = DEPART;
		_teleportsTable = TELEPORTS;
		_fuelTable = FUEL;
		_movie = Movie.LAND_KSERTH_B;
		validityCheck();
	}
	
	public static void main(String[] args)
	{
		new KeucereusNorthController();
	}
}
