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
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * @author Mobius
 */
public class AddSkillBySkill extends AbstractEffect
{
	private final int _existingSkillId;
	private final int _existingSkillLevel;
	private final SkillHolder _addedSkill;
	
	public AddSkillBySkill(StatSet params)
	{
		_existingSkillId = params.getInt("existingSkillId");
		_existingSkillLevel = params.getInt("existingSkillLevel");
		_addedSkill = new SkillHolder(params.getInt("addedSkillId"), params.getInt("addedSkillLevel"));
	}
	
	@Override
	public boolean canPump(Creature effector, Creature effected, Skill skill)
	{
		return effected.isPlayer() && !effected.isTransformed() && (effected.getSkillLevel(_existingSkillId) == _existingSkillLevel);
	}
	
	@Override
	public void pump(Creature effected, Skill skill)
	{
		final Player player = effected.asPlayer();
		player.addSkill(_addedSkill.getSkill(), false);
		
		ThreadPool.schedule(() ->
		{
			player.sendSkillList();
			player.getStat().recalculateStats(false);
			player.broadcastUserInfo();
		}, 100);
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		final Player player = effected.asPlayer();
		player.removeSkill(_addedSkill.getSkill(), false);
		
		ThreadPool.schedule(() ->
		{
			player.sendSkillList();
			player.getStat().recalculateStats(false);
			player.broadcastUserInfo();
		}, 100);
	}
}
