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
package ai.areas.HellboundIsland.Theorn;

import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.AbnormalType;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * Theorn AI
 * @author Gigi, Mobius
 * @date 2017-10-26 - [22:17:03]
 */
public class Theorn extends Script
{
	// NPC
	private static final int THERON = 33897;
	
	// Skills
	private static final int REWARD_BUFF_X2 = 16136;
	private static final int REWARD_BUFF_X4 = 16137;
	private static final int REWARD_BUFF_X8 = 16138;
	private static final int REWARD_BUFF_X16 = 16139;
	private static final int REWARD_BUFF_X32 = 16140;
	private static final Skill RESEARCH_SUCCESS_1 = SkillData.getInstance().getSkill(16141, 1);
	private static final Skill RESEARCH_SUCCESS_2 = SkillData.getInstance().getSkill(16142, 1);
	private static final Skill RESEARCH_SUCCESS_3 = SkillData.getInstance().getSkill(16143, 1);
	private static final Skill RESEARCH_SUCCESS_4 = SkillData.getInstance().getSkill(16144, 1);
	private static final Skill RESEARCH_SUCCESS_5 = SkillData.getInstance().getSkill(16145, 1);
	private static final Skill RESEARCH_FAIL = SkillData.getInstance().getSkill(16146, 1);
	
	private Theorn()
	{
		addStartNpc(THERON);
		addTalkId(THERON);
		addFirstTalkId(THERON);
		addSpawnId(THERON);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "BROADCAST_TEXT":
			{
				npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_WILL_LEAVE_THIS_PLACE_ONCE_DAY_BREAKS_IF_YOU_WANT_A_REWARD_HURRY_UP_AND_GET_IT_BE_CAREFUL_NOT_TO_LOSE_THE_RESEARCH_REWARDS_IN_THE_VOID);
				startQuestTimer("BROADCAST_TEXT", (60 + getRandom(10)) * 3000, npc, null);
				break;
			}
			case "light_1":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_1, true);
				break;
			}
			case "darkness_1":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_1, false);
				break;
			}
			case "light_2":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_2, true);
				break;
			}
			case "darkness_2":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_2, false);
				break;
			}
			case "light_3":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_3, true);
				break;
			}
			case "darkness_3":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_3, false);
				break;
			}
			case "light_4":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_4, true);
				break;
			}
			case "darkness_4":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_4, false);
				break;
			}
			case "light_5":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_5, true);
				break;
			}
			case "darkness_5":
			{
				htmltext = tryLuck(npc, player, RESEARCH_SUCCESS_5, false);
				break;
			}
			case "stop_1":
			{
				if ((player != null) && !player.isDead() && (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X2) != null))
				{
					addExpAndSp(player, 0, 1000000);
					showOnScreenMsg(player, NpcStringId.YOU_HAVE_ACQUIRED_SP_X_2, ExShowScreenMessage.TOP_CENTER, 5000);
					player.getEffectList().stopEffects(AbnormalType.STAR_AGATHION_EXP_SP_BUFF1);
				}
				break;
			}
			case "stop_2":
			{
				if ((player != null) && !player.isDead() && (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X4) != null))
				{
					addExpAndSp(player, 0, 2000000);
					showOnScreenMsg(player, NpcStringId.YOU_HAVE_ACQUIRED_SP_X_4, ExShowScreenMessage.TOP_CENTER, 5000);
					player.getEffectList().stopEffects(AbnormalType.STAR_AGATHION_EXP_SP_BUFF1);
				}
				break;
			}
			case "stop_3":
			{
				if ((player != null) && !player.isDead() && (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X8) != null))
				{
					addExpAndSp(player, 0, 4000000);
					showOnScreenMsg(player, NpcStringId.YOU_HAVE_ACQUIRED_SP_X_8, ExShowScreenMessage.TOP_CENTER, 5000);
					player.getEffectList().stopEffects(AbnormalType.STAR_AGATHION_EXP_SP_BUFF1);
				}
				break;
			}
			case "stop_4":
			{
				if ((player != null) && !player.isDead() && (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X16) != null))
				{
					addExpAndSp(player, 0, 8000000);
					showOnScreenMsg(player, NpcStringId.YOU_HAVE_ACQUIRED_SP_X_16, ExShowScreenMessage.TOP_CENTER, 5000);
					player.getEffectList().stopEffects(AbnormalType.STAR_AGATHION_EXP_SP_BUFF1);
				}
				break;
			}
			case "stop_5":
			{
				if ((player != null) && !player.isDead() && (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X32) != null))
				{
					addExpAndSp(player, 0, 16000000);
					showOnScreenMsg(player, NpcStringId.S1_ACQUIRED_32_TIMES_THE_SKILL_POINTS_AS_A_REWARD, ExShowScreenMessage.TOP_CENTER, 5000, player.getName());
					player.getEffectList().stopEffects(AbnormalType.STAR_AGATHION_EXP_SP_BUFF1);
				}
				break;
			}
			case "REMOVE_BUFF":
			{
				player.getEffectList().stopEffects(AbnormalType.STAR_AGATHION_EXP_SP_BUFF1);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		if (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X2) != null)
		{
			return "33897_x2.html";
		}
		else if (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X4) != null)
		{
			return "33897_x4.html";
		}
		else if (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X8) != null)
		{
			return "33897_x8.html";
		}
		else if (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X16) != null)
		{
			return "33897_x16.html";
		}
		else if (player.getEffectList().getBuffInfoBySkillId(REWARD_BUFF_X32) != null)
		{
			return "33897_x32.html";
		}
		
		return "33897.html";
	}
	
	private String tryLuck(Npc npc, Player player, Skill skill, boolean isLight)
	{
		if ((player.getSp() < 500000) || (player.getAdena() < 100000))
		{
			return "nosp.html";
		}
		
		takeItems(player, Inventory.ADENA_ID, 100000);
		player.setSp(player.getSp() - 500000);
		if (isLight ? getRandom(10) > 4 : getRandom(10) < 5)
		{
			SkillCaster.triggerCast(npc, player, skill);
			return null;
		}
		
		SkillCaster.triggerCast(npc, player, RESEARCH_FAIL);
		startQuestTimer("REMOVE_BUFF", 3000, npc, player);
		return null;
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("BROADCAST_TEXT", 3000, npc, null);
	}
	
	public static void main(String[] args)
	{
		new Theorn();
	}
}
