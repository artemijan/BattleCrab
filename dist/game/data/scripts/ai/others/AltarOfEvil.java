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

import org.l2jmobius.gameserver.model.actor.Attackable;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * Altar of Evil AI.
 * @author St3eT
 */
public class AltarOfEvil extends Script
{
	// NPCs
	private static final int RIFTER = 23179; // Dimensional Rifter
	
	// Skill
	private static final SkillHolder SKILL = new SkillHolder(14643, 1); // Summon
	
	public AltarOfEvil()
	{
		addAttackId(RIFTER);
		addNpcHateId(RIFTER);
		addSpellFinishedId(RIFTER);
	}
	
	@Override
	public boolean onNpcHate(Attackable mob, Player player, boolean isSummon)
	{
		teleportPlayer(mob, player);
		return super.onNpcHate(mob, player, isSummon);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		teleportPlayer(npc, attacker);
	}
	
	@Override
	public void onSpellFinished(Npc npc, Player player, Skill skill)
	{
		if (skill.getId() == SKILL.getSkillId())
		{
			showOnScreenMsg(player, NpcStringId.DIMENSIONAL_RIFTER_SUMMONED_YOU, ExShowScreenMessage.TOP_CENTER, 5000);
			player.teleToLocation(npc);
		}
	}
	
	private void teleportPlayer(Npc npc, Player player)
	{
		if (npc.isScriptValue(0) && (npc.calculateDistance3D(player) > 200))
		{
			addSkillCastDesire(npc, player, SKILL, 23);
			npc.setScriptValue(1);
		}
	}
	
	public static void main(String[] args)
	{
		new AltarOfEvil();
	}
}
