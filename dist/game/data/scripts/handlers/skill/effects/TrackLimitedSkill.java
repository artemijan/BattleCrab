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
import org.l2jmobius.gameserver.model.actor.holders.creature.EffectList;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.enums.SkillFinishType;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * @author Ofelin
 */
public class TrackLimitedSkill extends AbstractEffect
{
	private static final int LIMIT_OF_AEORE = 11833;
	private static final int LIMIT_OF_SIGEL = 19526;
	private static final int LIMIT_OF_ISS = 19527;
	private static final int BATTLE_RAPSODY = 11544;
	private static final int OVERLORDS_DIGNITY = 19439;
	private static final int PROTECTION_OF_FATE = 10019;
	private static final int NINE_AEGIS = 10024;
	private static final int CELESTIAL_PROTECTION = 11758;
	private static final int CELESTIAL_PARTY_PROTECTION = 11759;
	private int limitAeoreLevel = 0;
	private int limitSigelLevel = 0;
	private int limitIssLevel = 0;
	
	public TrackLimitedSkill(StatSet param)
	{
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		trackAeoreLimit(effector, effected, LIMIT_OF_AEORE); // Tracking Aeore Limit Debuff
		trackSigelLimit(effector, effected, LIMIT_OF_SIGEL); // Tracking Sigel Limit Debuff
		trackIssLimit(effector, effected, LIMIT_OF_ISS); // Tracking Iss Limit Debuff
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		if ((skill.getId() == BATTLE_RAPSODY) || (skill.getId() == OVERLORDS_DIGNITY))
		{
			increaseLimit(effector, effected, skill, limitIssLevel);
		}
		else if ((skill.getId() == PROTECTION_OF_FATE) || (skill.getId() == NINE_AEGIS))
		{
			increaseLimit(effector, effected, skill, limitSigelLevel);
		}
		else if ((skill.getId() == CELESTIAL_PROTECTION) || (skill.getId() == CELESTIAL_PARTY_PROTECTION))
		{
			increaseLimit(effector, effected, skill, limitAeoreLevel);
		}
	}
	
	private void trackAeoreLimit(Creature effector, Creature effected, int limitSkillId)
	{
		limitAeoreLevel = 0;
		final EffectList effectList = effected.getEffectList();
		for (BuffInfo debuff : effectList.getDebuffs())
		{
			if (debuff.getSkill().getId() == limitSkillId)
			{
				limitAeoreLevel = debuff.getSkill().getLevel();
				
				if (limitAeoreLevel == 3)
				{
					effected.getEffectList().remove(effectList.getBuffInfoBySkillId(CELESTIAL_PROTECTION), SkillFinishType.NORMAL, false, false); // Remove Celestial Protection
					effected.getEffectList().remove(effectList.getBuffInfoBySkillId(CELESTIAL_PARTY_PROTECTION), SkillFinishType.NORMAL, false, false); // Remove Celestial Party Protection
				}
				else
				{
					new SkillHolder(LIMIT_OF_AEORE, limitAeoreLevel).getSkill().applyEffects(effector, effected);
				}
			}
		}
	}
	
	private void trackSigelLimit(Creature effector, Creature effected, int limitSkillId)
	{
		limitSigelLevel = 0;
		final EffectList effectList = effected.getEffectList();
		for (BuffInfo debuff : effectList.getDebuffs())
		{
			if (debuff.getSkill().getId() == limitSkillId)
			{
				limitSigelLevel = debuff.getSkill().getLevel();
				
				if (limitSigelLevel == 3)
				{
					effected.getEffectList().remove(effectList.getBuffInfoBySkillId(PROTECTION_OF_FATE), SkillFinishType.NORMAL, false, false); // Remove Protection of Fate
					effected.getEffectList().remove(effectList.getBuffInfoBySkillId(NINE_AEGIS), SkillFinishType.NORMAL, false, false); // Remove Nine Aegis
				}
				else
				{
					new SkillHolder(LIMIT_OF_SIGEL, limitSigelLevel).getSkill().applyEffects(effector, effected);
				}
			}
		}
	}
	
	private void trackIssLimit(Creature effector, Creature effected, int limitSkillId)
	{
		limitIssLevel = 0;
		final EffectList effectList = effected.getEffectList();
		for (BuffInfo debuff : effectList.getDebuffs())
		{
			if (debuff.getSkill().getId() == limitSkillId)
			{
				limitIssLevel = debuff.getSkill().getLevel();
				if (limitIssLevel == 3)
				{
					effected.getEffectList().remove(effectList.getBuffInfoBySkillId(BATTLE_RAPSODY), SkillFinishType.NORMAL, false, false); // Remove Battle Rhapsody
					effected.getEffectList().remove(effectList.getBuffInfoBySkillId(OVERLORDS_DIGNITY), SkillFinishType.NORMAL, false, false); // Remove Overlord's Dignity
				}
				else
				{
					new SkillHolder(LIMIT_OF_ISS, limitIssLevel).getSkill().applyEffects(effector, effected);
				}
			}
		}
	}
	
	private void increaseLimit(Creature effector, Creature effected, Skill skill, int limitLevel)
	{
		if (limitLevel < 3)
		{
			switch (skill.getId())
			{
				case BATTLE_RAPSODY: // Battle Rhapsody
				case OVERLORDS_DIGNITY: // Overlord's Dignity
				{
					// limitIssLevel++;
					new SkillHolder(LIMIT_OF_ISS, ++limitIssLevel).getSkill().applyEffects(effector, effected);
					break;
				}
				case PROTECTION_OF_FATE: // Protection of Fate
				case NINE_AEGIS: // Nine Aegis
				{
					new SkillHolder(LIMIT_OF_SIGEL, ++limitSigelLevel).getSkill().applyEffects(effector, effected);
					break;
				}
				case CELESTIAL_PROTECTION: // Celestial Protection
				case CELESTIAL_PARTY_PROTECTION: // Celestial Party Protection
				{
					new SkillHolder(LIMIT_OF_AEORE, ++limitAeoreLevel).getSkill().applyEffects(effector, effected);
					break;
				}
			}
		}
	}
}
