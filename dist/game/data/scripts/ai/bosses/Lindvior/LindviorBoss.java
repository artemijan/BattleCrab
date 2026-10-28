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
package ai.bosses.Lindvior;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * LindviorBoss AI
 * @author Gigi
 * @date 2017-08-02 - [11:05:21]
 */
public class LindviorBoss extends Script
{
	// Boss
	private static final int LINDVIOR_GROUND = 25899;
	private static final int LINDVIOR_RAID = 29240;
	private static final int LINDVIOR_FLY = 19424;
	
	// Skills
	private static final SkillHolder MASS_HELL_BINDING = new SkillHolder(11052, 6);
	private static final SkillHolder MIGHTY_WIND_STRIKE = new SkillHolder(15274, 1);
	private static final SkillHolder WIND_PULL = new SkillHolder(15591, 1);
	private static final SkillHolder LINDVIORS_JUMP = new SkillHolder(15430, 1);
	private static final SkillHolder BODY_SLAM = new SkillHolder(15271, 1);
	private static final SkillHolder SOAR = new SkillHolder(15279, 1);
	private static final SkillHolder WIND_BREAT = new SkillHolder(15272, 1);
	private static final SkillHolder TAIL_SWIPE = new SkillHolder(15273, 1);
	private static final SkillHolder TORNADO = new SkillHolder(15275, 1);
	private static final SkillHolder LINDVIORS_ATTACK = new SkillHolder(15600, 1);
	
	// Chances
	private static final int CHANCE_WIND_PULL = 4;
	private static final int CHANCE_LINDVIORS_JUMP = 7;
	private static final int CHANCE_BODY_SLAM = 2;
	private static final int CHANCE_SOAR = 8;
	private static final int CHANCE_WIND_BREAT = 3;
	private static final int CHANCE_TAIL_SWIPE = 5;
	private static final int CHANCE_TORNADO = 6;
	private static final int CHANCE_LINDVIORS_ATTACK = 1;
	
	public LindviorBoss()
	{
		super();
		addAttackId(LINDVIOR_GROUND, LINDVIOR_FLY, LINDVIOR_RAID);
		addSpawnId(LINDVIOR_FLY);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if (!npc.isCastingNow())
		{
			final int chance = getRandom(100);
			switch (npc.getId())
			{
				case LINDVIOR_GROUND:
				{
					if (chance <= CHANCE_WIND_PULL)
					{
						npc.setTarget(attacker);
						npc.doCast(WIND_PULL.getSkill());
					}
					else if (chance <= CHANCE_LINDVIORS_JUMP)
					{
						npc.setTarget(attacker);
						npc.doCast(LINDVIORS_JUMP.getSkill());
					}
					else if (chance <= CHANCE_BODY_SLAM)
					{
						npc.setTarget(attacker);
						npc.doCast(BODY_SLAM.getSkill());
					}
					else if (chance <= CHANCE_TAIL_SWIPE)
					{
						npc.setTarget(attacker);
						npc.doCast(TAIL_SWIPE.getSkill());
					}
					else
					{
						npc.setTarget(attacker);
						npc.doCast(MIGHTY_WIND_STRIKE.getSkill());
					}
					break;
				}
				case LINDVIOR_FLY:
				{
					if (chance <= CHANCE_SOAR)
					{
						npc.setTarget(attacker);
						npc.doCast(SOAR.getSkill());
					}
					else
					{
						npc.setTarget(attacker);
						npc.doCast(MIGHTY_WIND_STRIKE.getSkill());
					}
					break;
				}
				case LINDVIOR_RAID:
				{
					if (chance <= CHANCE_WIND_BREAT)
					{
						npc.setTarget(attacker);
						npc.doCast(WIND_BREAT.getSkill());
					}
					else if (chance <= CHANCE_WIND_PULL)
					{
						npc.setTarget(attacker);
						npc.doCast(WIND_PULL.getSkill());
					}
					else if (chance <= CHANCE_TAIL_SWIPE)
					{
						npc.setTarget(attacker);
						npc.doCast(TAIL_SWIPE.getSkill());
					}
					else if (chance <= CHANCE_TORNADO)
					{
						npc.setTarget(attacker);
						npc.doCast(TORNADO.getSkill());
					}
					else if (chance <= CHANCE_LINDVIORS_ATTACK)
					{
						npc.setTarget(attacker);
						npc.doCast(LINDVIORS_ATTACK.getSkill());
					}
					else
					{
						npc.setTarget(attacker);
						npc.doCast(MIGHTY_WIND_STRIKE.getSkill());
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.setRandomWalking(true);
		npc.doCast(MASS_HELL_BINDING.getSkill());
	}
	
	public static void main(String[] args)
	{
		new LindviorBoss();
	}
}
