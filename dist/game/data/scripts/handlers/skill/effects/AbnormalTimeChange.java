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

import java.util.Collections;
import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.AbnormalType;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.serverpackets.AbnormalStatusUpdate;
import org.l2jmobius.gameserver.network.serverpackets.ExAbnormalStatusUpdateFromTarget;

/**
 * @author Sdw
 */
public class AbnormalTimeChange extends AbstractEffect
{
	private final Set<AbnormalType> _abnormals;
	private final int _time;
	private final int _mode;
	
	public AbnormalTimeChange(StatSet params)
	{
		final String abnormals = params.getString("slot", null);
		if ((abnormals != null) && !abnormals.isEmpty())
		{
			_abnormals = new HashSet<>();
			for (String slot : abnormals.split(";"))
			{
				_abnormals.add(AbnormalType.getAbnormalType(slot));
			}
		}
		else
		{
			_abnormals = Collections.<AbnormalType> emptySet();
		}
		
		_time = params.getInt("time", -1);
		
		switch (params.getString("mode", "DEBUFF"))
		{
			case "DIFF":
			{
				_mode = 0;
				break;
			}
			case "DEBUFF":
			{
				_mode = 1;
				break;
			}
			default:
			{
				throw new IllegalArgumentException("Mode should be DIFF or DEBUFF for skill id:" + params.getInt("id"));
			}
		}
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		final AbnormalStatusUpdate asu = new AbnormalStatusUpdate();
		
		switch (_mode)
		{
			case 0: // DIFF
			{
				if (_abnormals.isEmpty())
				{
					for (BuffInfo info : effected.getEffectList().getEffects())
					{
						if (info.getSkill().canBeDispelled())
						{
							info.resetAbnormalTime(info.getTime() + _time);
							asu.addSkill(info);
						}
					}
				}
				else
				{
					for (BuffInfo info : effected.getEffectList().getEffects())
					{
						if (info.getSkill().canBeDispelled() && _abnormals.contains(info.getSkill().getAbnormalType()))
						{
							info.resetAbnormalTime(info.getTime() + _time);
							asu.addSkill(info);
						}
					}
				}
				break;
			}
			case 1: // DEBUFF
			{
				if (_abnormals.isEmpty())
				{
					for (BuffInfo info : effected.getEffectList().getDebuffs())
					{
						if (info.getSkill().canBeDispelled())
						{
							info.resetAbnormalTime(info.getAbnormalTime());
							asu.addSkill(info);
						}
					}
				}
				else
				{
					for (BuffInfo info : effected.getEffectList().getDebuffs())
					{
						if (info.getSkill().canBeDispelled() && _abnormals.contains(info.getSkill().getAbnormalType()))
						{
							info.resetAbnormalTime(info.getAbnormalTime());
							asu.addSkill(info);
						}
					}
				}
				break;
			}
		}
		
		effected.sendPacket(asu);
		
		final ExAbnormalStatusUpdateFromTarget upd = new ExAbnormalStatusUpdateFromTarget(effected);
		for (Creature creature : effected.getStatus().getStatusListener())
		{
			if ((creature != null) && creature.isPlayer())
			{
				creature.sendPacket(upd);
			}
		}
		
		if (effected.isPlayer() && (effected.getTarget() == effected))
		{
			effected.sendPacket(upd);
		}
	}
}
