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
package ai.areas.ImperialTomb;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

/**
 * Simple AI for Bloody Succubus.
 * @author Gigi
 */
public class BloodySuccubus extends Script
{
	// NPCs
	private static final int BLOODY_SUCCUBUS = 23185;
	private static final int SUCCUBUS_OF_DEATH = 23191;
	private static final int SUCCUBUS_OF_DARKNESS = 23192;
	private static final int SUCCUBUS_OF_LUNACY = 23197;
	private static final int SUCCUBUS_OF_SILENCE = 23198;
	
	// Attack messages
	private static final NpcStringId[] ON_ATTACK_MSG_BLOODY_SUCCUBUS =
	{
		NpcStringId.HEY_THAT_HURT_YOU_JUST_WAIT_HERE_AND_I_LL_BE_BACK_AS_A_STRONGER_DRAGON,
		NpcStringId.HAHAHA_YOU_DARE_TO_DISRUPT_ME_I_WILL_BE_YOUR_NIGHTMARE_FROM_WHICH_YOU_CAN_NEVER_AWAKEN,
		NpcStringId.I_CANNOT_LET_YOU_STOP_THE_WRAITH_OF_SHILEN,
		NpcStringId.YOU_DARE_ATTACK_ME_I_WILL_FILL_YOUR_NIGHTMARES_WITH_BLOOD,
		NpcStringId.HALT_YOUR_NIGHTMARES_WILL_FILL_YOU_WITH_DREAD
	};
	private static final NpcStringId[] ON_ATTACK_MSG_SUCCUBUS_OF_DEATH =
	{
		NpcStringId.HOW_FOOLISH_THE_PRICE_OF_ATTACKING_ME_IS_DEATH,
		NpcStringId.PREPARE_I_SHALL_GRANT_YOU_DEATH,
		NpcStringId.MY_SWORD_WILL_TAKE_YOUR_LIFE,
		NpcStringId.OH_SHILEN_GIVE_ME_STRENGTH,
		NpcStringId.YAAAH
	};
	private static final NpcStringId[] ON_ATTACK_MSG_SUCCUBUS_OF_DARKNESS =
	{
		NpcStringId.HYAAAAAAH,
		NpcStringId.ARE_YOU_THE_ONE_TO_SHATTER_THE_PEACE,
		NpcStringId.OUR_MISSION_IS_TO_RESURRECT_THE_GODDESS_DO_NOT_INTERFERE,
		NpcStringId.FEEL_THE_TRUE_TERROR_OF_DARKNESS
	};
	private static final NpcStringId[] ON_ATTACK_MSG_SUCCUBUS_OF_LUNACY =
	{
		NpcStringId.HEHEHE_I_M_GLAD_YOU_CAME_I_WAS_HUNGRY,
		NpcStringId.SMALL_FRY_I_WILL_SHOW_YOU_TRUE_MADNESS_HAHAHA,
		NpcStringId.HEHEHE_PREPARE_MY_MADNESS_WILL_SWALLOW_YOU_UP,
		NpcStringId.HEHEHE_SHALL_WE_PLAY,
		NpcStringId.HEHEHE_PREPARE_MY_MADNESS_WILL_SWALLOW_YOU_UP
	};
	private static final NpcStringId[] ON_ATTACK_MSG_SUCCUBUS_OF_SILENCE =
	{
		NpcStringId.FOR_THE_GODDESS,
		NpcStringId.YOU_WILL_DIE,
		NpcStringId.DIE_2,
		NpcStringId.YOU_WILL_BE_DESTROYED,
		NpcStringId.OOOOH,
		NpcStringId.DO_NOT_INTERFERE
	};
	
	// Death messages
	private static final NpcStringId[] ON_FAILED_MSG_BLOODY_SUCCUBUS =
	{
		NpcStringId.AH,
		NpcStringId.SHILEN_I_HAVE_FAILED,
		NpcStringId.HOW_ALL_THAT_POWER_REMOVED,
		NpcStringId.TO_THINK_THAT_I_COULD_FAIL_IMPOSSIBLE
	};
	private static final NpcStringId[] ON_FAILED_MSG_SUCCUBUS_OF_DEATH =
	{
		NpcStringId.NO_I_LOST_ALL_THE_GATHERED_POWER_OF_LIGHT_TO_THIS_THIS,
		NpcStringId.I_WOULD_DEFEATED,
		NpcStringId.DON_T_THINK_THIS_IS_THE_END
	};
	private static final NpcStringId[] ON_FAILED_MSG_SUCCUBUS_OF_DARKNESS =
	{
		NpcStringId.I_WILL_ALWAYS_WATCH_YOU_FROM_THE_DARKNESS,
		NpcStringId.NO_N_NO_NO,
		NpcStringId.I_MUSTN_T_LOSE_THE_STRENGTH,
		NpcStringId.OH_CREATURES_OF_THE_GODDESS_LEND_ME_YOUR_STRENGTH
	};
	private static final NpcStringId[] ON_FAILED_MSG_SUCCUBUS_OF_LUNACY =
	{
		NpcStringId.ACK_NO_MY_BODY_IT_S_DISAPPEARING,
		NpcStringId.KYAAAH,
		NpcStringId.HUHUHU_HUHUHU_HUHAHAHA,
		NpcStringId.HUH_WHAT_HAPPENED_I_I_LOST
	};
	private static final NpcStringId[] ON_FAILED_MSG_SUCCUBUS_OF_SILENCE =
	{
		NpcStringId.IS_THIS_THE_END_2,
		NpcStringId.OH_GODDESS,
		NpcStringId.NO_I_DIDN_T_STAY_SILENT_ALL_THIS_TIME_JUST_TO_DISAPPEAR_NOW_LIKE_THIS
	};
	
	private BloodySuccubus()
	{
		addAttackId(BLOODY_SUCCUBUS, SUCCUBUS_OF_DEATH, SUCCUBUS_OF_DARKNESS, SUCCUBUS_OF_LUNACY, SUCCUBUS_OF_SILENCE);
		addKillId(BLOODY_SUCCUBUS, SUCCUBUS_OF_DEATH, SUCCUBUS_OF_DARKNESS, SUCCUBUS_OF_LUNACY, SUCCUBUS_OF_SILENCE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "say":
			{
				if (npc.isDead() || !npc.isScriptValue(1))
				{
					cancelQuestTimer("say", npc, null);
					return null;
				}
				
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_ATTACK_MSG_BLOODY_SUCCUBUS));
				break;
			}
			case "say1":
			{
				if (npc.isDead() || !npc.isScriptValue(1))
				{
					cancelQuestTimer("say1", npc, null);
					return null;
				}
				
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_ATTACK_MSG_SUCCUBUS_OF_DEATH));
				break;
			}
			case "say2":
			{
				if (npc.isDead() || !npc.isScriptValue(1))
				{
					cancelQuestTimer("say2", npc, null);
					return null;
				}
				
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_ATTACK_MSG_SUCCUBUS_OF_DARKNESS));
				break;
			}
			case "say3":
			{
				if (npc.isDead() || !npc.isScriptValue(1))
				{
					cancelQuestTimer("say3", npc, null);
					return null;
				}
				
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_ATTACK_MSG_SUCCUBUS_OF_LUNACY));
				break;
			}
			case "say4":
			{
				if (npc.isDead() || !npc.isScriptValue(1))
				{
					cancelQuestTimer("say4", npc, null);
					return null;
				}
				
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_ATTACK_MSG_SUCCUBUS_OF_SILENCE));
				break;
			}
		}
		
		return null;
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		switch (npc.getId())
		{
			case BLOODY_SUCCUBUS:
			{
				if (npc.isScriptValue(0))
				{
					npc.setScriptValue(1);
					startQuestTimer("say", (getRandom(5) + 3) * 1000, npc, null, true);
				}
				break;
			}
			case SUCCUBUS_OF_DEATH:
			{
				if (npc.isScriptValue(0))
				{
					npc.setScriptValue(1);
					startQuestTimer("say1", (getRandom(5) + 3) * 1000, npc, null, true);
				}
				break;
			}
			case SUCCUBUS_OF_DARKNESS:
			{
				if (npc.isScriptValue(0))
				{
					npc.setScriptValue(1);
					startQuestTimer("say2", (getRandom(5) + 3) * 1000, npc, null, true);
				}
				break;
			}
			case SUCCUBUS_OF_LUNACY:
			{
				if (npc.isScriptValue(0))
				{
					npc.setScriptValue(1);
					startQuestTimer("say3", (getRandom(5) + 3) * 1000, npc, null, true);
				}
				break;
			}
			case SUCCUBUS_OF_SILENCE:
			{
				if (npc.isScriptValue(0))
				{
					npc.setScriptValue(1);
					startQuestTimer("say4", (getRandom(5) + 3) * 1000, npc, null, true);
				}
				break;
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		switch (npc.getId())
		{
			case BLOODY_SUCCUBUS:
			{
				cancelQuestTimer("say", npc, player);
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_FAILED_MSG_BLOODY_SUCCUBUS));
				break;
			}
			case SUCCUBUS_OF_DEATH:
			{
				cancelQuestTimer("say1", npc, player);
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_FAILED_MSG_SUCCUBUS_OF_DEATH));
				break;
			}
			case SUCCUBUS_OF_DARKNESS:
			{
				cancelQuestTimer("say2", npc, player);
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_FAILED_MSG_SUCCUBUS_OF_DARKNESS));
				break;
			}
			case SUCCUBUS_OF_LUNACY:
			{
				cancelQuestTimer("say3", npc, player);
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_FAILED_MSG_SUCCUBUS_OF_LUNACY));
				break;
			}
			case SUCCUBUS_OF_SILENCE:
			{
				cancelQuestTimer("say4", npc, player);
				npc.broadcastSay(ChatType.NPC_GENERAL, getRandomEntry(ON_FAILED_MSG_SUCCUBUS_OF_SILENCE));
				break;
			}
		}
	}
	
	public static void main(String[] args)
	{
		new BloodySuccubus();
	}
}
