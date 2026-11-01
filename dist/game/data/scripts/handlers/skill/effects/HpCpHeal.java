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

import org.l2jmobius.gameserver.config.custom.ClassBalanceConfig;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.effects.EffectFlag;
import org.l2jmobius.gameserver.model.effects.EffectType;
import org.l2jmobius.gameserver.model.item.enums.ShotType;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.item.type.CrystalType;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Formulas;
import org.l2jmobius.gameserver.model.stats.Stat;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ExMagicAttackInfo;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * HpCpHeal effect implementation.
 * @author Sdw, Mobius
 */
public class HpCpHeal extends AbstractEffect
{
	private final double _power;
	
	public HpCpHeal(StatSet params)
	{
		_power = params.getDouble("power", 0);
		
		if (params.contains("amount"))
		{
			throw new IllegalArgumentException(getClass().getSimpleName() + " should use power instead of amount.");
		}
	}
	
	@Override
	public EffectType getEffectType()
	{
		return EffectType.HEAL;
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effected.isDead() || effected.isDoor() || effected.isHpBlocked())
		{
			return;
		}
		
		if ((effected != effector) && effected.isAffected(EffectFlag.FACEOFF))
		{
			return;
		}
		
		double amount = _power;
		double staticShotBonus = 0;
		double mAtkMul = 1;
		final boolean sps = skill.isMagic() && effector.isChargedShot(ShotType.SPIRITSHOTS);
		final boolean bss = skill.isMagic() && effector.isChargedShot(ShotType.BLESSED_SPIRITSHOTS);
		final double shotsBonus = effector.getStat().getValue(Stat.SHOTS_BONUS);
		
		if (((sps || bss) && (effector.isPlayer() && effector.asPlayer().isMageClass())) || effector.isSummon())
		{
			staticShotBonus = skill.getMpConsume(); // static bonus for spiritshots
			mAtkMul = bss ? 4 * shotsBonus : 2 * shotsBonus;
			staticShotBonus *= bss ? 2.4 : 1.0;
		}
		else if ((sps || bss) && effector.isNpc())
		{
			staticShotBonus = 2.4 * skill.getMpConsume(); // always blessed spiritshots
			mAtkMul = 4 * shotsBonus;
		}
		else
		{
			// no static bonus
			// grade dynamic bonus
			final Item weaponInst = effector.getActiveWeaponInstance();
			if (weaponInst != null)
			{
				mAtkMul = weaponInst.getTemplate().getCrystalTypePlus() == CrystalType.R ? 4 : weaponInst.getTemplate().getCrystalTypePlus() == CrystalType.S ? 2 : 1;
			}
			
			// shot dynamic bonus
			mAtkMul = bss ? mAtkMul * 4 : mAtkMul + 1;
		}
		
		if (!skill.isStatic())
		{
			amount += staticShotBonus + Math.sqrt(mAtkMul * effector.getMAtk());
			amount *= effected.getStat().getValue(Stat.HEAL_EFFECT, 1);
			amount += effected.getStat().getValue(Stat.HEAL_EFFECT_ADD, 0);
			amount *= (item == null) && effector.isPlayable() ? ClassBalanceConfig.PLAYER_HEALING_SKILL_MULTIPLIERS[effector.asPlayer().getPlayerClass().getId()] : 1f;
			
			// Heal critic, since CT2.3 Gracia Final
			if (skill.isMagic() && (Formulas.calcCrit(skill.getMagicCriticalRate(), effector, effected, skill) || effector.isAffected(EffectFlag.HPCPHEAL_CRITICAL)))
			{
				amount *= 3;
				effector.sendPacket(SystemMessageId.M_CRITICAL);
				effector.sendPacket(new ExMagicAttackInfo(effector.getObjectId(), effected.getObjectId(), ExMagicAttackInfo.CRITICAL_HEAL));
				if (effected.isPlayer() && (effected != effector))
				{
					effected.sendPacket(new ExMagicAttackInfo(effector.getObjectId(), effected.getObjectId(), ExMagicAttackInfo.CRITICAL_HEAL));
				}
			}
		}
		
		// Additional potion HP.
		double additionalHp = 0;
		
		// Additional potion CP.
		double additionalCp = 0;
		
		if ((item != null) && (item.isPotion() || item.isElixir()))
		{
			additionalHp = effected.getStat().getValue(Stat.ADDITIONAL_POTION_HP, 0);
			additionalCp = effected.getStat().getValue(Stat.ADDITIONAL_POTION_CP, 0);
		}
		
		// Prevents overheal and negative amount
		final double healAmount = Math.max(Math.min(amount, effected.getMaxRecoverableHp() - effected.getCurrentHp()), 0);
		if (healAmount != 0)
		{
			final double newHp = healAmount + effected.getCurrentHp();
			if ((newHp + additionalHp) > effected.getMaxRecoverableHp())
			{
				additionalHp = Math.max(effected.getMaxRecoverableHp() - newHp, 0);
			}
			
			effected.setCurrentHp(newHp + additionalHp, false);
			
			if (effector.isPlayer() && (effector != effected))
			{
				final SystemMessage sm = new SystemMessage(SystemMessageId.S2_HP_HAS_BEEN_RESTORED_BY_C1);
				sm.addString(effector.getName());
				sm.addInt((int) (healAmount + additionalHp));
				effected.sendPacket(sm);
			}
			else
			{
				final SystemMessage sm = new SystemMessage(SystemMessageId.S1_HP_HAS_BEEN_RESTORED);
				sm.addInt((int) (healAmount + additionalHp));
				effected.sendPacket(sm);
			}
		}
		
		// CP recovery.
		if (effected.isPlayer())
		{
			amount = Math.max(Math.min(amount - healAmount, effected.getMaxRecoverableCp() - effected.getCurrentCp()), 0);
			if (amount != 0)
			{
				final double newCp = amount + effected.getCurrentCp();
				if ((newCp + additionalCp) > effected.getMaxRecoverableCp())
				{
					additionalCp = Math.max(effected.getMaxRecoverableCp() - newCp, 0);
				}
				
				effected.setCurrentCp(newCp + additionalCp, false);
				
				if (effector != effected)
				{
					final SystemMessage sm = new SystemMessage(SystemMessageId.S2_CP_HAS_BEEN_RESTORED_BY_C1);
					sm.addString(effector.getName());
					sm.addInt((int) (amount + additionalCp));
					effected.sendPacket(sm);
				}
				else
				{
					final SystemMessage sm = new SystemMessage(SystemMessageId.S1_CP_HAS_BEEN_RESTORED);
					sm.addInt((int) (amount + additionalCp));
					effected.sendPacket(sm);
				}
			}
		}
		
		effected.broadcastStatusUpdate(effector);
	}
}
