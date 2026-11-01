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
package ai.bosses.Frintezza;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * @author Micr0, Zerox, Mobius
 */
public class ScarletVanHalisha extends Script
{
	// NPCs
	private static final int HALISHA2 = 29046;
	private static final int HALISHA3 = 29047;
	
	// Skills
	private static final int FRINTEZZA_DAEMON_ATTACK = 5014;
	private static final int FRINTEZZA_DAEMON_CHARGE = 5015;
	private static final int YOKE_OF_SCARLET = 5016;
	private static final int FRINTEZZA_DAEMON_MORPH = 5018;
	private static final int FRINTEZZA_DAEMON_FIELD = 5019;
	
	// Misc
	private static final int RANGED_SKILL_MIN_COOLTIME = 60000; // 1 minute
	private Creature _target;
	private Skill _skill;
	private long _lastRangedSkillTime;
	
	public ScarletVanHalisha()
	{
		addAttackId(HALISHA2, HALISHA3);
		addKillId(HALISHA2, HALISHA3);
		addSpellFinishedId(HALISHA2, HALISHA3);
		addAttackId(HALISHA2, HALISHA3);
		addKillId(HALISHA2, HALISHA3);
		addSpellFinishedId(HALISHA2, HALISHA3);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "ATTACK":
			{
				if (npc != null)
				{
					getSkillAI(npc);
				}
				break;
			}
			case "RANDOM_TARGET":
			{
				_target = getRandomTarget(npc, null);
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onSpellFinished(Npc npc, Player player, Skill skill)
	{
		getSkillAI(npc);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		startQuestTimer("RANDOM_TARGET", 5000, npc, null, true);
		startQuestTimer("ATTACK", 500, npc, null, true);
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		cancelQuestTimers("ATTACK");
		cancelQuestTimers("RANDOM_TARGET");
	}
	
	private Skill getRndSkills(Npc npc)
	{
		switch (npc.getId())
		{
			case HALISHA2:
			{
				if (getRandom(100) < 10)
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_CHARGE, 2);
				}
				else if (getRandom(100) < 10)
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_CHARGE, 5);
				}
				else if (getRandom(100) < 2)
				{
					return SkillData.getInstance().getSkill(YOKE_OF_SCARLET, 1);
				}
				else
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_ATTACK, 2);
				}
			}
			case HALISHA3:
			{
				if (getRandom(100) < 10)
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_CHARGE, 3);
				}
				else if (getRandom(100) < 10)
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_CHARGE, 6);
				}
				else if (getRandom(100) < 10)
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_CHARGE, 2);
				}
				else if (((_lastRangedSkillTime + RANGED_SKILL_MIN_COOLTIME) < System.currentTimeMillis()) && (getRandom(100) < 10))
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_FIELD, 1);
				}
				else if (((_lastRangedSkillTime + RANGED_SKILL_MIN_COOLTIME) < System.currentTimeMillis()) && (getRandom(100) < 10))
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_MORPH, 1);
				}
				else if (getRandom(100) < 2)
				{
					return SkillData.getInstance().getSkill(YOKE_OF_SCARLET, 1);
				}
				else
				{
					return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_ATTACK, 3);
				}
			}
		}
		
		return SkillData.getInstance().getSkill(FRINTEZZA_DAEMON_ATTACK, 1);
	}
	
	private synchronized void getSkillAI(Npc npc)
	{
		if (npc.isInvul() || npc.isCastingNow())
		{
			return;
		}
		
		if ((getRandom(100) < 30) || (_target == null) || _target.isDead())
		{
			_skill = getRndSkills(npc);
			_target = getRandomTarget(npc, _skill);
		}
		
		Skill skill = _skill;
		if (skill == null)
		{
			skill = getRndSkills(npc);
		}
		
		if (npc.isPhysicalMuted())
		{
			return;
		}
		
		final Creature target = _target;
		if ((target == null) || target.isDead())
		{
			// npc.setCastingNow(false);
			return;
		}
		
		if (LocationUtil.checkIfInRange(skill.getCastRange(), npc, target, true))
		{
			npc.getAI().setIntention(Intention.IDLE);
			npc.setTarget(target);
			// npc.setCastingNow(true);
			_target = null;
			npc.doCast(skill);
		}
		else
		{
			npc.getAI().setIntention(Intention.FOLLOW, target, null);
			npc.getAI().setIntention(Intention.ATTACK, target, null);
			// npc.setCastingNow(false);
		}
	}
	
	private Creature getRandomTarget(Npc npc, Skill skill)
	{
		final List<Creature> result = new ArrayList<>();
		for (Player obj : npc.getInstanceWorld().getPlayers())
		{
			if (obj.isPlayer() && obj.asPlayer().isInvisible())
			{
				continue;
			}
			
			if (((obj.getZ() < (npc.getZ() - 100)) && (obj.getZ() > (npc.getZ() + 100))) || !GeoEngine.getInstance().canSeeTarget(obj, npc))
			{
				continue;
			}
			
			int skillRange = 150;
			if (skill != null)
			{
				switch (skill.getId())
				{
					case FRINTEZZA_DAEMON_ATTACK:
					{
						skillRange = 150;
						break;
					}
					case FRINTEZZA_DAEMON_CHARGE:
					{
						skillRange = 400;
						break;
					}
					case YOKE_OF_SCARLET:
					{
						skillRange = 200;
						break;
					}
					case FRINTEZZA_DAEMON_MORPH:
					case FRINTEZZA_DAEMON_FIELD:
					{
						_lastRangedSkillTime = System.currentTimeMillis();
						skillRange = 550;
						break;
					}
				}
				
				if (LocationUtil.checkIfInRange(skillRange, npc, obj, true) && !obj.asCreature().isDead())
				{
					result.add(obj);
				}
			}
		}
		
		return getRandomEntry(result);
	}
	
	public static void main(String[] args)
	{
		new ScarletVanHalisha();
	}
}
