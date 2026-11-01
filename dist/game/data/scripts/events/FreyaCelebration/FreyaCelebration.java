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
package events.FreyaCelebration;

import java.util.Collection;

import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.script.LongTimeEvent;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.CreatureSay;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Freya Celebration event AI.
 * @author Gnacik
 */
public class FreyaCelebration extends LongTimeEvent
{
	// NPC
	private static final int FREYA = 13296;
	
	// Items
	private static final int FREYA_POTION = 15440;
	private static final int FREYA_GIFT = 17138;
	
	// Misc
	private static final String RESET_VAR = "FreyaCelebration";
	private static final int HOURS = 20;
	
	private static final int[] SKILLS =
	{
		9150,
		9151,
		9152,
		9153,
		9154,
		9155,
		9156
	};
	
	private static final NpcStringId[] FREYA_TEXT =
	{
		NpcStringId.EVEN_THOUGH_YOU_BRING_SOMETHING_CALLED_A_GIFT_AMONG_YOUR_HUMANS_IT_WOULD_JUST_BE_PROBLEMATIC_FOR_ME,
		NpcStringId.I_JUST_DON_T_KNOW_WHAT_EXPRESSION_I_SHOULD_HAVE_IT_APPEARED_ON_ME_ARE_HUMAN_S_EMOTIONS_LIKE_THIS_FEELING,
		NpcStringId.THE_FEELING_OF_THANKS_IS_JUST_TOO_MUCH_DISTANT_MEMORY_FOR_ME,
		NpcStringId.BUT_I_KIND_OF_MISS_IT_LIKE_I_HAD_FELT_THIS_FEELING_BEFORE,
		NpcStringId.I_AM_ICE_QUEEN_FREYA_THIS_FEELING_AND_EMOTION_ARE_NOTHING_BUT_A_PART_OF_MALISS_A_MEMORIES
	};
	
	private FreyaCelebration()
	{
		addStartNpc(FREYA);
		addFirstTalkId(FREYA);
		addTalkId(FREYA);
		addSkillSeeId(FREYA);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equalsIgnoreCase("give_potion"))
		{
			if (getQuestItemsCount(player, Inventory.ADENA_ID) > 1)
			{
				final long currentTime = System.currentTimeMillis();
				final long reuseTime = player.getVariables().getLong(RESET_VAR, 0);
				if (currentTime > reuseTime)
				{
					takeItems(player, Inventory.ADENA_ID, 1);
					giveItems(player, FREYA_POTION, 1);
					player.getVariables().set(RESET_VAR, currentTime + (HOURS * 3600000));
				}
				else
				{
					final long remainingTime = (reuseTime - currentTime) / 1000;
					final int hours = (int) (remainingTime / 3600);
					final int minutes = (int) ((remainingTime % 3600) / 60);
					final SystemMessage sm = new SystemMessage(SystemMessageId.S1_WILL_BE_AVAILABLE_FOR_RE_USE_AFTER_S2_HR_S3_MIN);
					sm.addItemName(FREYA_POTION);
					sm.addInt(hours);
					sm.addInt(minutes);
					player.sendPacket(sm);
				}
			}
			else
			{
				final SystemMessage sm = new SystemMessage(SystemMessageId.YOU_NEED_S2_S1_S);
				sm.addItemName(Inventory.ADENA_ID);
				sm.addInt(1);
				player.sendPacket(sm);
			}
		}
		
		return null;
	}
	
	@Override
	public void onSkillSee(Npc npc, Player caster, Skill skill, Collection<WorldObject> targets, boolean isSummon)
	{
		if ((caster == null) || (npc == null))
		{
			return;
		}
		
		if ((npc.getId() == FREYA) && targets.contains(npc) && ArrayUtil.contains(SKILLS, skill.getId()))
		{
			if (getRandom(100) < 5)
			{
				final CreatureSay cs = new CreatureSay(npc, ChatType.NPC_GENERAL, NpcStringId.DEAR_S1_THINK_OF_THIS_AS_MY_APPRECIATION_FOR_THE_GIFT_TAKE_THIS_WITH_YOU_THERE_S_NOTHING_STRANGE_ABOUT_IT_IT_S_JUST_A_BIT_OF_MY_CAPRICIOUSNESS);
				cs.addStringParameter(caster.getName());
				npc.broadcastPacket(cs);
				
				caster.addItem(ItemProcessType.REWARD, FREYA_GIFT, 1, npc, true);
			}
			else if (getRandom(10) < 2)
			{
				npc.broadcastPacket(new CreatureSay(npc, ChatType.NPC_GENERAL, getRandomEntry(FREYA_TEXT)));
			}
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "13296.htm";
	}
	
	public static void main(String[] args)
	{
		new FreyaCelebration();
	}
}
