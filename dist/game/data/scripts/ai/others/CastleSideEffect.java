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
package ai.others;

import org.l2jmobius.gameserver.managers.CastleManager;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.siege.Castle;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.network.serverpackets.ExCastleState;

/**
 * Shows castle side effect in cities.
 * @author Gigi
 * @date 2019-05-14 - [12:47:33]
 */
public class CastleSideEffect extends Script
{
	private static final int[] ZONE_ID =
	{
		11020, // Giran
		11027, // Gludio
		11028, // Dion
		11029, // Oren
		11031, // aden
		11032, // Goddard
		11033, // Rune
		11034, // Heine
		11035, // Shuttgard
	};
	
	public CastleSideEffect()
	{
		addEnterZoneId(ZONE_ID);
	}
	
	@Override
	public void onEnterZone(Creature character, ZoneType zone)
	{
		if (character.isPlayer())
		{
			for (Castle castle : CastleManager.getInstance().getCastles())
			{
				character.sendPacket(new ExCastleState(castle));
			}
		}
	}
	
	public static void main(String[] args)
	{
		new CastleSideEffect();
	}
}
