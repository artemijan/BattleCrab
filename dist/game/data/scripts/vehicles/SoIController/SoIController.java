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
package vehicles.SoIController;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.holders.creature.VehiclePathPoint;
import org.l2jmobius.gameserver.network.enums.Movie;

import vehicles.AirShipController;

public class SoIController extends AirShipController
{
	private static final int DOCK_ZONE = 50600;
	private static final int LOCATION = 101;
	private static final int CONTROLLER_ID = 32604;
	
	private static final VehiclePathPoint[] ARRIVAL =
	{
		new VehiclePathPoint(-214422, 211396, 5000, 280, 2000),
		new VehiclePathPoint(-214422, 211396, 4422, 280, 2000)
	};
	
	private static final VehiclePathPoint[] DEPART =
	{
		new VehiclePathPoint(-214422, 211396, 5000, 280, 2000),
		new VehiclePathPoint(-215877, 209709, 5000, 280, 2000)
	};
	
	private static final VehiclePathPoint[][] TELEPORTS =
	{
		{
			new VehiclePathPoint(-214422, 211396, 5000, 280, 2000),
			new VehiclePathPoint(-215877, 209709, 5000, 280, 2000),
			new VehiclePathPoint(-206692, 220997, 3000, 0, 0)
		},
		{
			new VehiclePathPoint(-214422, 211396, 5000, 280, 2000),
			new VehiclePathPoint(-215877, 209709, 5000, 280, 2000),
			new VehiclePathPoint(-195357, 233430, 2500, 0, 0)
		}
	};
	
	private static final int[] FUEL =
	{
		0,
		50
	};
	
	public SoIController()
	{
		addStartNpc(CONTROLLER_ID);
		addFirstTalkId(CONTROLLER_ID);
		addTalkId(CONTROLLER_ID);
		
		_dockZone = DOCK_ZONE;
		addEnterZoneId(DOCK_ZONE);
		addExitZoneId(DOCK_ZONE);
		
		_shipSpawnX = -212719;
		_shipSpawnY = 213348;
		_shipSpawnZ = 5000;
		_oustLoc = new Location(-213401, 210401, 4408);
		_locationId = LOCATION;
		_arrivalPath = ARRIVAL;
		_departPath = DEPART;
		_teleportsTable = TELEPORTS;
		_fuelTable = FUEL;
		_movie = Movie.LAND_UNDEAD_A;
		validityCheck();
	}
	
	public static void main(String[] args)
	{
		new SoIController();
	}
}
