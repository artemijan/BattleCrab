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

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.player.ShortcutType;
import org.l2jmobius.gameserver.model.actor.holders.player.Shortcut;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.serverpackets.AbnormalStatusUpdate;
import org.l2jmobius.gameserver.network.serverpackets.ShortcutInit;
import org.l2jmobius.gameserver.network.serverpackets.ShortcutRegister;

/**
 * @author Mobius
 */
public class ReplaceSkillBySkill extends AbstractEffect
{
	private final SkillHolder _existingSkill;
	private final SkillHolder _replacementSkill;
	
	public ReplaceSkillBySkill(StatSet params)
	{
		_existingSkill = new SkillHolder(params.getInt("existingSkillId"), params.getInt("existingSkillLevel", -1));
		_replacementSkill = new SkillHolder(params.getInt("replacementSkillId"), params.getInt("replacementSkillLevel", -1));
	}
	
	@Override
	public boolean canStart(Creature effector, Creature effected, Skill skill)
	{
		return effected.isPlayer() && !effected.isTransformed();
	}
	
	@Override
	public boolean canPump(Creature effector, Creature effected, Skill skill)
	{
		return (skill != null) && skill.isPassive() && effected.isPlayer() && !effected.isTransformed();
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		applyEffect(effector, effected, skill, item);
	}
	
	@Override
	public void pump(Creature effected, Skill skill)
	{
		applyEffect(effected, effected, skill, null);
	}
	
	private void applyEffect(Creature effector, Creature effected, Skill skill, Item item)
	{
		final Player player = effected.asPlayer();
		final Skill knownSkill = player.getKnownSkill(_existingSkill.getSkillId());
		if ((knownSkill == null) || (knownSkill.getLevel() < _existingSkill.getSkillLevel()))
		{
			return;
		}
		
		final Skill addedSkill = SkillData.getInstance().getSkill(_replacementSkill.getSkillId(), _replacementSkill.getSkillLevel() < 1 ? knownSkill.getLevel() : _replacementSkill.getSkillLevel(), knownSkill.getSubLevel());
		player.addSkill(addedSkill, false);
		player.addReplacedSkill(_existingSkill.getSkillId());
		for (Shortcut shortcut : player.getAllShortcuts())
		{
			if ((shortcut.getType() == ShortcutType.SKILL) && (shortcut.getId() == knownSkill.getId()) && (shortcut.getLevel() == knownSkill.getLevel()))
			{
				final int slot = shortcut.getSlot();
				final int page = shortcut.getPage();
				final int characterType = shortcut.getCharacterType();
				player.deleteShortcut(slot, page);
				final Shortcut newShortcut = new Shortcut(slot, page, ShortcutType.SKILL, addedSkill.getId(), addedSkill.getLevel(), addedSkill.getSubLevel(), characterType);
				player.registerShortcut(newShortcut);
				player.sendPacket(new ShortcutRegister(newShortcut, player));
			}
		}
		
		// Replace continuous effects.
		if (knownSkill.isContinuous() && player.isAffectedBySkill(knownSkill.getId()))
		{
			int abnormalTime = 0;
			for (BuffInfo info : player.getEffectList().getEffects())
			{
				if (info.getSkill().getId() == knownSkill.getId())
				{
					abnormalTime = info.getAbnormalTime();
					break;
				}
			}
			
			if (abnormalTime > 2000)
			{
				addedSkill.applyEffects(player, player);
				final AbnormalStatusUpdate asu = new AbnormalStatusUpdate();
				for (BuffInfo info : player.getEffectList().getEffects())
				{
					if (info.getSkill().getId() == addedSkill.getId())
					{
						info.resetAbnormalTime(abnormalTime);
						asu.addSkill(info);
					}
				}
				
				player.sendPacket(asu);
			}
		}
		
		player.removeSkill(knownSkill, false);
		player.sendSkillList();
		ThreadPool.schedule(() -> player.sendPacket(new ShortcutInit(player)), 1100);
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		final Player player = effected.asPlayer();
		final Skill knownSkill = player.getKnownSkill(_replacementSkill.getSkillId());
		if (knownSkill == null)
		{
			return;
		}
		
		final Skill addedSkill = SkillData.getInstance().getSkill(_existingSkill.getSkillId(), _existingSkill.getSkillLevel() < 1 ? knownSkill.getLevel() : _existingSkill.getSkillLevel(), knownSkill.getSubLevel());
		player.addSkill(addedSkill, false);
		player.removeReplacedSkill(_existingSkill.getSkillId());
		for (Shortcut shortcut : player.getAllShortcuts())
		{
			if ((shortcut.getType() == ShortcutType.SKILL) && (shortcut.getId() == knownSkill.getId()) && (shortcut.getLevel() == knownSkill.getLevel()))
			{
				final int slot = shortcut.getSlot();
				final int page = shortcut.getPage();
				final int characterType = shortcut.getCharacterType();
				player.deleteShortcut(slot, page);
				final Shortcut newShortcut = new Shortcut(slot, page, ShortcutType.SKILL, addedSkill.getId(), addedSkill.getLevel(), addedSkill.getSubLevel(), characterType);
				player.registerShortcut(newShortcut);
				player.sendPacket(new ShortcutRegister(newShortcut, player));
			}
		}
		
		// Replace continuous effects.
		if (knownSkill.isContinuous() && player.isAffectedBySkill(knownSkill.getId()))
		{
			int abnormalTime = 0;
			for (BuffInfo info : player.getEffectList().getEffects())
			{
				if (info.getSkill().getId() == knownSkill.getId())
				{
					abnormalTime = info.getAbnormalTime();
					break;
				}
			}
			
			if (abnormalTime > 2000)
			{
				addedSkill.applyEffects(player, player);
				final AbnormalStatusUpdate asu = new AbnormalStatusUpdate();
				for (BuffInfo info : player.getEffectList().getEffects())
				{
					if (info.getSkill().getId() == addedSkill.getId())
					{
						info.resetAbnormalTime(abnormalTime);
						asu.addSkill(info);
					}
				}
				
				player.sendPacket(asu);
			}
		}
		
		player.removeSkill(knownSkill, false);
		player.sendSkillList();
		ThreadPool.schedule(() -> player.sendPacket(new ShortcutInit(player)), 1100);
	}
}
