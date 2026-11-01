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
package ai.areas.OrbisTemple;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.enums.SkillFinishType;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * Orbis Temple Statue AI.
 * @author Mobius
 */
public class OrbisTempleStatues extends Script
{
	// NPCs
	private static final int VICTIM_1 = 22911;
	private static final int VICTIM_2 = 22912;
	private static final int VICTIM_3 = 22913;
	private static final int GUARD_1 = 22914;
	private static final int GUARD_2 = 22915;
	private static final int GUARD_3 = 22916;
	private static final int THROWER_1 = 22917;
	private static final int THROWER_2 = 22918;
	private static final int THROWER_3 = 22919;
	private static final int ANCIENT_HERO = 22925;
	private static final int CURATOR = 22923;
	private static final int CHIEF_CURATOR = 22927;
	
	// Items
	private static final int SWORD = 15280;
	private static final int SPEAR = 17372;
	
	// Skill
	private static final SkillHolder SPAWN_SKILL = new SkillHolder(14277, 1); // Orbis Spawn
	private static final SkillHolder DESPAWN_SKILL = new SkillHolder(14278, 1); // Orbis De-spawn
	
	public OrbisTempleStatues()
	{
		addAttackId(VICTIM_1, VICTIM_2, VICTIM_3, GUARD_1, GUARD_2, GUARD_3, THROWER_1, THROWER_2, THROWER_3, CURATOR);
		addMoveFinishedId(VICTIM_1, VICTIM_2, VICTIM_3, GUARD_1, GUARD_2, GUARD_3, THROWER_1, THROWER_2, THROWER_3, CURATOR, ANCIENT_HERO, CHIEF_CURATOR);
		addSpawnId(VICTIM_1, VICTIM_2, VICTIM_3, GUARD_1, GUARD_2, GUARD_3, THROWER_1, THROWER_2, THROWER_3, CURATOR, ANCIENT_HERO, CHIEF_CURATOR);
		addKillId(VICTIM_1, VICTIM_2, VICTIM_3, GUARD_1, GUARD_2, GUARD_3, THROWER_1, THROWER_2, THROWER_3, CURATOR, ANCIENT_HERO, CHIEF_CURATOR);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isPet)
	{
		switch (npc.getId())
		{
			case VICTIM_1:
			case VICTIM_2:
			case VICTIM_3:
			case GUARD_1:
			case GUARD_2:
			case GUARD_3:
			case CURATOR:
			{
				if (npc.isImmobilized())
				{
					npc.setImmobilized(false);
					npc.setRHandId(SWORD);
				}
				break;
			}
			case THROWER_1:
			case THROWER_2:
			case THROWER_3:
			{
				if (npc.isImmobilized())
				{
					npc.setImmobilized(false);
					npc.setRHandId(SPEAR);
				}
				break;
			}
		}
	}
	
	@Override
	public void onMoveFinished(Npc npc)
	{
		if ((npc.getTarget() == null) && (npc.getTemplate().getRHandId() == 0))
		{
			npc.setRHandId(0);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.setRandomWalking(false);
		if (npc.getId() < ANCIENT_HERO)
		{
			npc.setImmobilized(true);
		}
		
		if (npc.isMonster())
		{
			npc.stopSkillEffects(SkillFinishType.SILENT, DESPAWN_SKILL.getSkillId());
			SkillCaster.triggerCast(npc, npc, SPAWN_SKILL.getSkill());
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (npc.isMonster())
		{
			SkillCaster.triggerCast(killer, npc, DESPAWN_SKILL.getSkill());
		}
	}
	
	public static void main(String[] args)
	{
		new OrbisTempleStatues();
	}
}
