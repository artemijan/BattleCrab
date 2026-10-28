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
package events.ThePowerOfLove;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.LongTimeEvent;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * The Power Of Love
 * @URL http://www.lineage2.com/en/news/events/02102016-the-power-of-love-part-iii.php
 * @author hlwrave
 */
public class ThePowerOfLove extends LongTimeEvent
{
	// NPC
	private static final int COCO = 33893;
	
	// Items
	private static final int CT = 37705;
	private static final int CT_TRANSORM = 37708;
	private static final int CT_SUMMON = 37711;
	private static final int CH = 37706;
	private static final int CH_TRANSORM = 37709;
	private static final int CH_SUMMON = 37712;
	private static final int CC = 37707;
	private static final int CC_TRANSORM = 37710;
	private static final int CC_SUMMON = 37713;
	
	// Skill
	private static final SkillHolder COCO_M = new SkillHolder(17155, 1); // Coco's Magic
	
	private ThePowerOfLove()
	{
		addStartNpc(COCO);
		addFirstTalkId(COCO);
		addTalkId(COCO);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "33893-1.htm":
			case "33893-2.htm":
			case "33893-3.htm":
			{
				htmltext = event;
				break;
			}
			case "coco_buff":
			{
				npc.setTarget(player);
				npc.doCast(COCO_M.getSkill());
				htmltext = "33893-4.htm";
				break;
			}
			case "ct":
			{
				if (!ownsAtLeastOneItem(player, CT) && !ownsAtLeastOneItem(player, CH) && !ownsAtLeastOneItem(player, CC))
				{
					giveItems(player, CT, 1);
					giveItems(player, CT_TRANSORM, 1);
					giveItems(player, CT_SUMMON, 1);
					htmltext = "33893-5.htm";
				}
				else
				{
					htmltext = "33893-9.htm";
				}
				break;
			}
			case "ch":
			{
				if (!ownsAtLeastOneItem(player, CT) && !ownsAtLeastOneItem(player, CH) && !ownsAtLeastOneItem(player, CC))
				{
					giveItems(player, CH, 1);
					giveItems(player, CH_TRANSORM, 1);
					giveItems(player, CH_SUMMON, 1);
					htmltext = "33893-6.htm";
				}
				else
				{
					htmltext = "33893-9.htm";
				}
				break;
			}
			case "cc":
			{
				if (!ownsAtLeastOneItem(player, CT) && !ownsAtLeastOneItem(player, CH) && !ownsAtLeastOneItem(player, CC))
				{
					giveItems(player, CC, 1);
					giveItems(player, CC_TRANSORM, 1);
					giveItems(player, CC_SUMMON, 1);
					htmltext = "33893-7.htm";
				}
				else
				{
					htmltext = "33893-9.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + "-1.htm";
	}
	
	public static void main(String[] args)
	{
		new ThePowerOfLove();
	}
}
