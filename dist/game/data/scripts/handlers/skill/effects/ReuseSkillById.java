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

import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.serverpackets.SkillCoolTime;

/**
 * @author Mobius
 */
public class ReuseSkillById extends AbstractEffect
{
	private final int _skillId;
	private final int _amount;
	
	public ReuseSkillById(StatSet params)
	{
		_skillId = params.getInt("skillId", 0);
		_amount = params.getInt("amount", 0);
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		final Player player = effector.asPlayer();
		if (player != null)
		{
			final int skillLevel = player.getSkillLevel(_skillId);
			if (skillLevel > 0)
			{
				final Skill s = SkillData.getInstance().getSkill(_skillId, skillLevel);
				if (s != null)
				{
					if (_amount > 0)
					{
						final long reuse = player.getSkillRemainingReuseTime(s.getReuseHashCode());
						if (reuse > 0)
						{
							player.removeTimeStamp(s);
							player.addTimeStamp(s, Math.max(0, reuse - _amount));
							player.sendPacket(new SkillCoolTime(player));
						}
					}
					else
					{
						player.removeTimeStamp(s);
						player.enableSkill(s);
						player.sendPacket(new SkillCoolTime(player));
					}
				}
			}
		}
	}
}
