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
package events.RudolphsBlessing;

import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.Containers;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.impl.creature.player.OnPlayerSummonAgathion;
import org.l2jmobius.gameserver.model.events.impl.creature.player.OnPlayerUnsummonAgathion;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.holders.ItemHolder;
import org.l2jmobius.gameserver.model.holders.SkillHolder;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.quest.LongTimeEvent;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;

/**
 * @author IvanTotov, Edoo
 */
public class RudolphsBlessing extends LongTimeEvent
{
	private static final int SANTA_CLAUS = 13285;
	
	private static final int AGATHION_SEAL_BRACELET_RUDOLPH = 21709;
	private static final int AGATHION_SEAL_BRACELET_RUDOLPH_NPC = 1598;
	private static final int ICE_CANDY_PIECE = 21915;
	private static final int SANTA_CLAUS_TREASURE_BOX = 21873;
	
	private static final int RUDOLPH_PRICE = 2019; // Calendar.getInstance().get(Calendar.YEAR) can be used but need to replace htmls.
	
	private static final SkillHolder RUDOLPH_TRICK = new SkillHolder(23181, 1);
	private static final ItemHolder ITEM_REQUIREMENT = new ItemHolder(21872, 2);
	
	public RudolphsBlessing()
	{
		addStartNpc(SANTA_CLAUS);
		addFirstTalkId(SANTA_CLAUS);
		addTalkId(SANTA_CLAUS);
		Containers.Global().addListener(new ConsumerEventListener(Containers.Global(), EventType.ON_PLAYER_SUMMON_AGATHION, (OnPlayerSummonAgathion event) -> onPlayerSummonAgathion(event), this));
		Containers.Global().addListener(new ConsumerEventListener(Containers.Global(), EventType.ON_PLAYER_UNSUMMON_AGATHION, (OnPlayerUnsummonAgathion event) -> onPlayerUnsummonAgathion(event), this));
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("rudolph_eat"))
		{
			if ((player != null) && player.isOnline())
			{
				if (takeItems(player, ITEM_REQUIREMENT.getId(), ITEM_REQUIREMENT.getCount()))
				{
					SkillCaster.triggerCast(player, player, RUDOLPH_TRICK.getSkill());
					final Skill rudolphsBlessing = SkillData.getInstance().getSkill(23297, 1);
					rudolphsBlessing.applyEffects(player, player);
				}
				startQuestTimer("rudolph_eat", 10 * 60 * 1000, null, player);
			}
		}
		if (player == null)
		{
			return null;
		}
		
		String htmltext = event;
		switch (event)
		{
			case "rudolph":
			{
				if (hasQuestItems(player, AGATHION_SEAL_BRACELET_RUDOLPH))
				{
					htmltext = "13285-05.htm";
				}
				else if (player.getAdena() < RUDOLPH_PRICE)
				{
					htmltext = "13285-06.htm";
				}
				else
				{
					takeItems(player, Inventory.ADENA_ID, RUDOLPH_PRICE);
					giveItems(player, AGATHION_SEAL_BRACELET_RUDOLPH, 1);
					htmltext = "13285-04.htm";
				}
				break;
			}
			case "event_info":
			{
				htmltext = "13285-02.htm";
				break;
			}
			case "reward":
			{
				if (getQuestItemsCount(player, ICE_CANDY_PIECE) >= 50)
				{
					takeItems(player, ICE_CANDY_PIECE, 50);
					giveItems(player, SANTA_CLAUS_TREASURE_BOX, 1);
				}
				else
				{
					htmltext = "13285-03.htm";
				}
				break;
			}
		}
		return htmltext;
	}
	
	private void onPlayerSummonAgathion(OnPlayerSummonAgathion event)
	{
		if (event.getAgathionId() != AGATHION_SEAL_BRACELET_RUDOLPH_NPC)
		{
			return;
		}
		final Player player = event.getPlayer();
		if (player == null)
		{
			return;
		}
		
		startQuestTimer("rudolph_eat", 10 * 60 * 1000, null, player);
	}
	
	private void onPlayerUnsummonAgathion(OnPlayerUnsummonAgathion event)
	{
		if (event.getAgathionId() != AGATHION_SEAL_BRACELET_RUDOLPH_NPC)
		{
			return;
		}
		final Player player = event.getPlayer();
		if (player == null)
		{
			return;
		}
		
		cancelQuestTimer("rudolph_eat", null, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "13285.htm";
	}
	
	public static void main(String[] args)
	{
		new RudolphsBlessing();
	}
}
