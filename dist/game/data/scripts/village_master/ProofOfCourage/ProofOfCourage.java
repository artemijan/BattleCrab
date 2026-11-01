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
package village_master.ProofOfCourage;

import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.l2jmobius.gameserver.data.xml.MultisellData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Proof Of Courage implementation.
 * @author St3eT
 */
public class ProofOfCourage extends Script
{
	// Misc
	private static final Map<Integer, List<PlayerClass>> CLASSLIST = new HashMap<>();
	static
	{
		CLASSLIST.put(32146, Arrays.asList(PlayerClass.TROOPER, PlayerClass.WARDER));
		CLASSLIST.put(32147, Arrays.asList(PlayerClass.ELVEN_KNIGHT, PlayerClass.ELVEN_SCOUT, PlayerClass.ELVEN_WIZARD, PlayerClass.ORACLE));
		CLASSLIST.put(32150, Arrays.asList(PlayerClass.ORC_RAIDER, PlayerClass.ORC_MONK));
		CLASSLIST.put(32153, Arrays.asList(PlayerClass.WARRIOR, PlayerClass.KNIGHT, PlayerClass.ROGUE, PlayerClass.WIZARD, PlayerClass.CLERIC));
		CLASSLIST.put(32157, Arrays.asList(PlayerClass.SCAVENGER, PlayerClass.ARTISAN));
		CLASSLIST.put(32160, Arrays.asList(PlayerClass.PALUS_KNIGHT, PlayerClass.ASSASSIN, PlayerClass.DARK_WIZARD, PlayerClass.SHILLIEN_ORACLE));
	}
	
	private ProofOfCourage()
	{
		addStartNpc(CLASSLIST.keySet());
		addTalkId(CLASSLIST.keySet());
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		if (talker.getPlayerClass().level() == 0)
		{
			return npc.getId() + "-noclass.html";
		}
		else if (!CLASSLIST.get(npc.getId()).contains(talker.getPlayerClass()))
		{
			return npc.getId() + "-no.html";
		}
		
		MultisellData.getInstance().separateAndSend(717, talker, npc, false);
		return super.onTalk(npc, talker);
	}
	
	public static void main(String[] args)
	{
		new ProofOfCourage();
	}
}
