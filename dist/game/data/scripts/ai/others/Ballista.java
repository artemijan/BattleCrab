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
package ai.others;

import java.util.Collection;

import org.l2jmobius.gameserver.config.FeatureConfig;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.SystemMessageId;

/**
 * Ballista AI.
 * @author St3eT
 */
public class Ballista extends Script
{
	// NPCs
	private static final int[] BALLISTA =
	{
		35685, // Shanty Fortress
		35723, // Southern Fortress
		35754, // Hive Fortress
		35792, // Valley Fortress
		35823, // Ivory Fortress
		35854, // Narsell Fortress
		35892, // Bayou Fortress
		35923, // White Sands Fortress
		35961, // Borderland Fortress
		35999, // Swamp Fortress
		36030, // Archaic Fortress
		36068, // Floran Fortress
		36106, // Cloud Mountain)
		36137, // Tanor Fortress
		36168, // Dragonspine Fortress
		36206, // Antharas's Fortress
		36244, // Western Fortress
		36282, // Hunter's Fortress
		36313, // Aaru Fortress
		36351, // Demon Fortress
		36389, // Monastic Fortress
	};
	
	// Skill
	private static final SkillHolder BOMB = new SkillHolder(2342, 1); // Ballista Bomb
	// Misc
	private static final int MIN_CLAN_LV = 5;
	
	private Ballista()
	{
		addSkillSeeId(BALLISTA);
		addSpawnId(BALLISTA);
	}
	
	@Override
	public void onSkillSee(Npc npc, Player caster, Skill skill, Collection<WorldObject> targets, boolean isSummon)
	{
		if ((skill != null) && (caster.getTarget() == npc) && (getRandom(100) < 40) && (skill == BOMB.getSkill()))
		{
			if (npc.getFort().getSiege().isInProgress() && (caster.getClan() != null) && (caster.getClan().getLevel() >= MIN_CLAN_LV))
			{
				caster.getClan().addReputationScore(FeatureConfig.BALLISTA_POINTS);
				caster.sendPacket(SystemMessageId.THE_BALLISTA_HAS_BEEN_SUCCESSFULLY_DESTROYED_THE_CLAN_REPUTATION_WILL_BE_INCREASED);
			}
			
			npc.doDie(caster);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.disableCoreAI(true);
		npc.setUndying(true);
	}
	
	public static void main(String[] args)
	{
		new Ballista();
	}
}
