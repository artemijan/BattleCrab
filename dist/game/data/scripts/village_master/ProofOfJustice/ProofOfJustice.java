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
package village_master.ProofOfJustice;

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
 * Proof Of Justice implementation.
 * @author St3eT
 */
public class ProofOfJustice extends Script
{
	// Items
	private static final int JUSTICE = 17822; // Proof of Justice
	
	// Misc
	private static final Map<Integer, List<PlayerClass>> CLASSLIST = new HashMap<>();
	static
	{
		CLASSLIST.put(30505, Arrays.asList(PlayerClass.DESTROYER, PlayerClass.TYRANT, PlayerClass.OVERLORD, PlayerClass.WARCRYER));
		CLASSLIST.put(30504, Arrays.asList(PlayerClass.BOUNTY_HUNTER, PlayerClass.WARSMITH));
		CLASSLIST.put(30288, Arrays.asList(PlayerClass.GLADIATOR, PlayerClass.WARLORD, PlayerClass.PALADIN, PlayerClass.DARK_AVENGER, PlayerClass.TREASURE_HUNTER, PlayerClass.HAWKEYE));
		CLASSLIST.put(30297, Arrays.asList(PlayerClass.SHILLIEN_KNIGHT, PlayerClass.BLADEDANCER, PlayerClass.ABYSS_WALKER, PlayerClass.PHANTOM_RANGER, PlayerClass.SPELLHOWLER, PlayerClass.PHANTOM_SUMMONER, PlayerClass.SHILLIEN_ELDER));
		CLASSLIST.put(30158, Arrays.asList(PlayerClass.SPELLSINGER, PlayerClass.ELEMENTAL_SUMMONER, PlayerClass.ELDER));
		CLASSLIST.put(30155, Arrays.asList(PlayerClass.TEMPLE_KNIGHT, PlayerClass.SWORDSINGER, PlayerClass.PLAINS_WALKER, PlayerClass.SILVER_RANGER));
		CLASSLIST.put(30289, Arrays.asList(PlayerClass.SORCERER, PlayerClass.NECROMANCER, PlayerClass.WARLOCK, PlayerClass.BISHOP, PlayerClass.PROPHET));
		CLASSLIST.put(32196, Arrays.asList(PlayerClass.BERSERKER, PlayerClass.MALE_SOULBREAKER, PlayerClass.FEMALE_SOULBREAKER, PlayerClass.ARBALESTER));
	}
	
	private ProofOfJustice()
	{
		addStartNpc(CLASSLIST.keySet());
		addTalkId(CLASSLIST.keySet());
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		if (player.getPlayerClass().level() < 2)
		{
			return npc.getId() + "-noclass.html";
		}
		else if (!hasAtLeastOneQuestItem(player, JUSTICE))
		{
			return npc.getId() + "-noitem.html";
		}
		else if (!CLASSLIST.get(npc.getId()).contains(player.getPlayerClass()))
		{
			return npc.getId() + "-no.html";
		}
		
		MultisellData.getInstance().separateAndSend(718, player, npc, false);
		return super.onTalk(npc, player);
	}
	
	public static void main(String[] args)
	{
		new ProofOfJustice();
	}
}
