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
package handlers.skill.effects;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.enums.DamageByAttackType;
import org.l2jmobius.gameserver.model.skill.enums.StatModifierType;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * An effect that changes damage taken from an attack.<br>
 * The retail implementation seems to be altering whatever damage is taken after the attack has been done and not when attack is being done.<br>
 * Exceptions for this effect appears to be DOT effects and terrain damage, they are unaffected by this stat.<br>
 * As for example in retail this effect does reduce reflected damage taken (because it is received damage), as well as it does not decrease reflected damage done,<br>
 * because reflected damage is being calculated with the original attack damage and not this altered one.<br>
 * Multiple values of this effect add-up to each other rather than multiplying with each other. Be careful, there were cases in retail where damage is deacreased to 0.
 * @author Nik, Mobius
 */
public class DamageByAttack extends AbstractEffect
{
	private final double _value;
	private final DamageByAttackType _type;
	
	public DamageByAttack(StatSet params)
	{
		_value = params.getDouble("amount");
		_type = params.getEnum("type", DamageByAttackType.class, DamageByAttackType.NONE);
		
		if (params.getEnum("mode", StatModifierType.class, StatModifierType.DIFF) != StatModifierType.DIFF)
		{
			throw new IllegalArgumentException(getClass().getSimpleName() + " can only use DIFF mode.");
		}
	}
	
	@Override
	public void pump(Creature target, Skill skill)
	{
		switch (_type)
		{
			case PK:
			{
				target.getStat().mergeAdd(Stat.PVP_DAMAGE_TAKEN, _value);
				break;
			}
			case ENEMY_ALL:
			{
				target.getStat().mergeAdd(Stat.PVE_DAMAGE_TAKEN, _value);
				break;
			}
			case MOB:
			{
				target.getStat().mergeAdd(Stat.PVE_DAMAGE_TAKEN_MONSTER, _value);
				break;
			}
			case BOSS:
			{
				target.getStat().mergeAdd(Stat.PVE_DAMAGE_TAKEN_RAID, _value);
				break;
			}
		}
	}
}
