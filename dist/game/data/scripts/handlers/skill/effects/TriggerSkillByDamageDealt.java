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
package handlers.skill.effects;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.logging.Level;

import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.handler.TargetHandler;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.enums.creature.InstanceType;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureDamageDealt;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.item.type.WeaponType;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.enums.SkillFinishType;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.skill.targets.TargetType;

/**
 * Trigger skill by damage dealt effect implementation.
 * @author Zealar
 */
public class TriggerSkillByDamageDealt extends AbstractEffect
{
	private final int _minAttackerLevel;
	private final int _maxAttackerLevel;
	private final int _minDamage;
	private final int _chance;
	private final SkillHolder _skill;
	private final TargetType _targetType;
	private final InstanceType _attackerType;
	private int _allowWeapons;
	private final boolean _isCritical;
	private final boolean _allowNormalAttack;
	private final boolean _allowSkillAttack;
	private final boolean _allowReflect;
	private final int _skillLevelScaleTo;
	private final List<SkillHolder> _triggerSkills;
	
	public TriggerSkillByDamageDealt(StatSet params)
	{
		_minAttackerLevel = params.getInt("minAttackerLevel", 1);
		_maxAttackerLevel = params.getInt("maxAttackerLevel", Integer.MAX_VALUE);
		_minDamage = params.getInt("minDamage", 1);
		_chance = params.getInt("chance", 100);
		_skill = new SkillHolder(params.getInt("skillId", 0), params.getInt("skillLevel", 1));
		_targetType = params.getEnum("targetType", TargetType.class, TargetType.SELF);
		_attackerType = params.getEnum("attackerType", InstanceType.class, InstanceType.Creature);
		_isCritical = params.getBoolean("isCritical", false);
		_allowNormalAttack = params.getBoolean("allowNormalAttack", true);
		_allowSkillAttack = params.getBoolean("allowSkillAttack", false);
		_allowReflect = params.getBoolean("allowReflect", false);
		_skillLevelScaleTo = params.getInt("skillLevelScaleTo", 0);
		
		if (params.getString("allowWeapons", "ALL").equalsIgnoreCase("ALL"))
		{
			_allowWeapons = 0;
		}
		else
		{
			for (String s : params.getString("allowWeapons").split(","))
			{
				_allowWeapons |= WeaponType.valueOf(s).mask();
			}
		}
		
		// Specific skills by level.
		final String triggerSkills = params.getString("triggerSkills", "");
		if (triggerSkills.isEmpty())
		{
			_triggerSkills = null;
		}
		else
		{
			final String[] split = triggerSkills.split(";");
			_triggerSkills = new ArrayList<>(split.length);
			for (String skill : split)
			{
				final String[] splitSkill = skill.split(",");
				_triggerSkills.add(new SkillHolder(Integer.parseInt(splitSkill[0]), Integer.parseInt(splitSkill[1])));
			}
		}
	}
	
	private void onAttackEvent(OnCreatureDamageDealt event)
	{
		if (event.isDamageOverTime() || (_chance == 0) || ((_triggerSkills == null) && ((_skill.getSkillId() == 0) || (_skill.getSkillLevel() == 0))) || (!_allowNormalAttack && !_allowSkillAttack))
		{
			return;
		}
		
		// Check if there is dependancy on critical.
		if (_isCritical != event.isCritical())
		{
			return;
		}
		
		// When no skill attacks are allowed.
		if (!_allowSkillAttack && (event.getSkill() != null))
		{
			return;
		}
		
		// When no normal attacks are allowed.
		if (!_allowNormalAttack && (event.getSkill() == null))
		{
			return;
		}
		
		if (!_allowReflect && event.isReflect())
		{
			return;
		}
		
		if (event.getAttacker() == event.getTarget())
		{
			return;
		}
		
		if ((event.getAttacker().getLevel() < _minAttackerLevel) || (event.getAttacker().getLevel() > _maxAttackerLevel))
		{
			return;
		}
		
		if ((event.getDamage() < _minDamage) || (Rnd.get(100) > _chance) || !event.getAttacker().getInstanceType().isType(_attackerType))
		{
			return;
		}
		
		if ((_allowWeapons > 0) && ((event.getAttacker().getActiveWeaponItem() == null) || ((event.getAttacker().getActiveWeaponItem().getItemType().mask() & _allowWeapons) == 0)))
		{
			return;
		}
		
		WorldObject target = null;
		try
		{
			target = TargetHandler.getInstance().getHandler(_targetType).getTarget(event.getAttacker(), event.getTarget(), _triggerSkills == null ? _skill.getSkill() : _triggerSkills.get(0).getSkill(), false, false, false);
		}
		catch (Exception e)
		{
			LOGGER.log(Level.WARNING, "Exception in ITargetTypeHandler.getTarget(): " + e.getMessage(), e);
		}
		
		if ((target == null) || !target.isCreature())
		{
			return;
		}
		
		Skill triggerSkill = null;
		if (_triggerSkills == null)
		{
			final BuffInfo buffInfo = target.asCreature().getEffectList().getBuffInfoBySkillId(_skill.getSkillId());
			if ((_skillLevelScaleTo <= 0) || (buffInfo == null))
			{
				triggerSkill = _skill.getSkill();
			}
			else
			{
				triggerSkill = SkillData.getInstance().getSkill(_skill.getSkillId(), Math.min(_skillLevelScaleTo, buffInfo.getSkill().getLevel() + 1));
				
				if (event.getAttacker().isSkillDisabled(buffInfo.getSkill()))
				{
					return;
				}
			}
			
			if ((buffInfo == null) || (buffInfo.getSkill().getLevel() < triggerSkill.getLevel()))
			{
				SkillCaster.triggerCast(event.getAttacker(), target.asCreature(), triggerSkill);
			}
		}
		else // Multiple trigger skills.
		{
			final Iterator<SkillHolder> iterator = _triggerSkills.iterator();
			while (iterator.hasNext())
			{
				final Skill nextSkill = iterator.next().getSkill();
				if (target.asCreature().isAffectedBySkill(nextSkill.getId()))
				{
					if (iterator.hasNext())
					{
						target.asCreature().stopSkillEffects(SkillFinishType.SILENT, nextSkill.getId());
						triggerSkill = iterator.next().getSkill();
						break;
					}
					
					// Already at last skill.
					return;
				}
			}
			
			if (triggerSkill == null)
			{
				triggerSkill = _triggerSkills.get(0).getSkill();
			}
			
			SkillCaster.triggerCast(event.getAttacker(), target.asCreature(), triggerSkill);
		}
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		effected.removeListenerIf(EventType.ON_CREATURE_DAMAGE_DEALT, listener -> listener.getOwner() == this);
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		effected.addListener(new ConsumerEventListener(effected, EventType.ON_CREATURE_DAMAGE_DEALT, (OnCreatureDamageDealt event) -> onAttackEvent(event), this));
	}
}
