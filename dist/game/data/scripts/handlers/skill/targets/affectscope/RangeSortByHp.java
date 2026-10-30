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
package handlers.skill.targets.affectscope;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Consumer;
import java.util.function.Predicate;

import org.l2jmobius.gameserver.handler.AffectObjectHandler;
import org.l2jmobius.gameserver.handler.IAffectObjectHandler;
import org.l2jmobius.gameserver.handler.IAffectScopeHandler;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.targets.AffectScope;

/**
 * Range sorted by lowest to highest hp percent affect scope implementation.
 * @author Nik, Mobius
 */
public class RangeSortByHp implements IAffectScopeHandler
{
	@Override
	public void forEachAffected(Creature creature, WorldObject target, Skill skill, Consumer<? super WorldObject> action)
	{
		final IAffectObjectHandler affectObject = AffectObjectHandler.getInstance().getHandler(skill.getAffectObject());
		final int affectRange = skill.getAffectRange();
		final int affectLimit = skill.getAffectLimit();
		
		// Target checks.
		final AtomicInteger affected = new AtomicInteger(0);
		final Predicate<Creature> filter = c ->
		{
			if ((affectLimit > 0) && (affected.get() >= affectLimit))
			{
				return false;
			}
			
			if (c.isDead())
			{
				return false;
			}
			
			// Range skills appear to not affect you unless you are the main target.
			if ((c == creature) && (target != creature))
			{
				return false;
			}
			
			if ((affectObject != null) && !affectObject.checkAffectedObject(creature, c))
			{
				return false;
			}
			
			affected.incrementAndGet();
			return true;
		};
		
		final List<Creature> result = World.getInstance().getVisibleObjectsInRange(target, Creature.class, affectRange, filter);
		
		// Add object of origin since it is skipped in the getVisibleObjects method.
		if (target.isCreature() && filter.test(target.asCreature()))
		{
			result.add(target.asCreature());
		}
		
		// Sort from lowest hp to highest hp.
		final List<Creature> sortedList = new ArrayList<>(result);
		Collections.sort(sortedList, Comparator.comparingInt(Creature::getCurrentHpPercent));
		
		int count = 0;
		final int limit = (affectLimit > 0) ? affectLimit : Integer.MAX_VALUE;
		for (Creature c : sortedList)
		{
			if (count >= limit)
			{
				break;
			}
			
			count++;
			action.accept(c);
		}
	}
	
	@Override
	public Enum<AffectScope> getAffectScopeType()
	{
		return AffectScope.RANGE_SORT_BY_HP;
	}
}
