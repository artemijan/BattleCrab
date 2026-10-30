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
package handlers.items;

import java.util.List;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.handler.IItemHandler;
import org.l2jmobius.gameserver.model.actor.Playable;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.effects.EffectType;
import org.l2jmobius.gameserver.model.item.enums.ItemSkillType;
import org.l2jmobius.gameserver.model.item.holders.ItemSkillHolder;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.skill.targets.TargetType;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * Template for item skills handler.
 * @author Zoey76
 */
public class ItemSkillsTemplate implements IItemHandler
{
	@Override
	public boolean onItemUse(Playable playable, Item item, boolean forceUse)
	{
		if (!playable.isPlayer() && !playable.isPet())
		{
			return false;
		}
		
		// Pets can use items only when they are tradable.
		if (playable.isPet() && !item.isTradeable())
		{
			playable.sendPacket(SystemMessageId.YOUR_PET_CANNOT_CARRY_THIS_ITEM);
			return false;
		}
		
		// Verify that item is not under reuse.
		if (!checkReuse(playable, null, item))
		{
			return false;
		}
		
		final List<ItemSkillHolder> skills = item.getTemplate().getSkills(ItemSkillType.NORMAL);
		if (skills == null)
		{
			LOGGER.info("Item " + item + " does not have registered any skill for handler.");
			return false;
		}
		
		boolean hasConsumeSkill = false;
		boolean successfulUse = false;
		for (SkillHolder skillInfo : skills)
		{
			if (skillInfo == null)
			{
				continue;
			}
			
			final Skill itemSkill = skillInfo.getSkill();
			if (itemSkill != null)
			{
				if (itemSkill.hasEffectType(EffectType.EXTRACT_ITEM))
				{
					final Player player = playable.asPlayer();
					if ((player != null) && !player.isInventoryUnder80(false))
					{
						player.sendPacket(SystemMessageId.YOU_VE_EXCEEDED_THE_LIMIT_AND_CANNOT_RETRIEVE_THE_ITEM_PLEASE_CHECK_YOUR_LIMIT_IN_THE_INVENTORY);
						return false;
					}
				}
				
				if (itemSkill.getItemConsumeId() > 0)
				{
					hasConsumeSkill = true;
				}
				
				if (!itemSkill.hasEffectType(EffectType.SUMMON_PET) && !itemSkill.checkCondition(playable, playable.getTarget()))
				{
					continue;
				}
				
				// Verify that skill is not under reuse.
				if (!checkReuse(playable, itemSkill, item))
				{
					continue;
				}
				
				if (playable.isSkillDisabled(itemSkill))
				{
					continue;
				}
				
				if (!item.isPotion() && !item.isElixir() && !item.isScroll() && playable.isCastingNow())
				{
					continue;
				}
				
				// Send message to the master.
				if (playable.isPet())
				{
					final SystemMessage sm = new SystemMessage(SystemMessageId.YOUR_PET_USES_S1);
					sm.addSkillName(itemSkill);
					playable.sendPacket(sm);
				}
				
				if (playable.isPlayer() && itemSkill.hasEffectType(EffectType.SUMMON_PET))
				{
					playable.doCast(itemSkill);
					successfulUse = true;
				}
				else if (itemSkill.isWithoutAction() || item.getTemplate().hasImmediateEffect() || item.getTemplate().hasExImmediateEffect())
				{
					SkillCaster.triggerCast(playable, itemSkill.getTargetType() == TargetType.OTHERS ? playable.getTarget() : null, itemSkill, item, false);
					successfulUse = true;
				}
				else
				{
					playable.getAI().setIntention(Intention.IDLE);
					if (playable.useMagic(itemSkill, item, forceUse, false))
					{
						successfulUse = true;
					}
					else
					{
						continue;
					}
				}
				
				if (itemSkill.getReuseDelay() > 0)
				{
					playable.addTimeStamp(itemSkill, itemSkill.getReuseDelay());
				}
			}
		}
		
		if (successfulUse && checkConsume(item, hasConsumeSkill))
		{
			if (!playable.destroyItem(null, item.getObjectId(), 1, playable, false))
			{
				playable.sendPacket(SystemMessageId.INCORRECT_ITEM_COUNT_2);
				return false;
			}
		}
		
		return successfulUse;
	}
	
	/**
	 * @param item the item being used
	 * @param hasConsumeSkill
	 * @return {@code true} check if item use consume item, {@code false} otherwise
	 */
	private boolean checkConsume(Item item, boolean hasConsumeSkill)
	{
		switch (item.getTemplate().getDefaultAction())
		{
			case CAPSULE:
			case SKILL_REDUCE:
			{
				if (!hasConsumeSkill && item.getTemplate().hasImmediateEffect())
				{
					return true;
				}
				break;
			}
			case SKILL_REDUCE_ON_SKILL_SUCCESS:
			{
				return false;
			}
		}
		
		return hasConsumeSkill;
	}
	
	/**
	 * @param playable the character using the item or skill
	 * @param skill the skill being used, can be null
	 * @param item the item being used
	 * @return {@code true} if the the item or skill to check is available, {@code false} otherwise
	 */
	private boolean checkReuse(Playable playable, Skill skill, Item item)
	{
		final long remainingTime = (skill != null) ? playable.getSkillRemainingReuseTime(skill.getReuseHashCode()) : playable.getItemRemainingReuseTime(item.getObjectId());
		final boolean isAvailable = remainingTime <= 0;
		if (playable.isPlayer())
		{
			if (!isAvailable)
			{
				final int hours = (int) (remainingTime / 3600000);
				final int minutes = (int) (remainingTime % 3600000) / 60000;
				final int seconds = (int) ((remainingTime / 1000) % 60);
				SystemMessage sm = null;
				if (hours > 0)
				{
					sm = new SystemMessage(SystemMessageId.THERE_ARE_S2_HR_S3_MIN_AND_S4_SEC_REMAINING_IN_S1_S_RE_USE_TIME);
					if ((skill == null) || skill.isStatic())
					{
						sm.addItemName(item);
					}
					else
					{
						sm.addSkillName(skill);
					}
					
					sm.addInt(hours);
					sm.addInt(minutes);
				}
				else if (minutes > 0)
				{
					sm = new SystemMessage(SystemMessageId.THERE_ARE_S2_MIN_S3_SEC_REMAINING_IN_S1_S_RE_USE_TIME);
					if ((skill == null) || skill.isStatic())
					{
						sm.addItemName(item);
					}
					else
					{
						sm.addSkillName(skill);
					}
					
					sm.addInt(minutes);
				}
				else
				{
					sm = new SystemMessage(SystemMessageId.THERE_ARE_S2_SEC_REMAINING_IN_S1_S_RE_USE_TIME);
					if ((skill == null) || skill.isStatic())
					{
						sm.addItemName(item);
					}
					else
					{
						sm.addSkillName(skill);
					}
				}
				
				sm.addInt(seconds);
				playable.sendPacket(sm);
			}
		}
		
		return isAvailable;
	}
}
