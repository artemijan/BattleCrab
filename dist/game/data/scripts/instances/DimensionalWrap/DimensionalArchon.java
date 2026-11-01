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
package instances.DimensionalWrap;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * Dimensional Archon AI
 * @author Gigi
 * @date 2018-09-08 - [18:09:02]
 */
public class DimensionalArchon extends Script
{
	// NPCs
	private static final int DEMINSIONAL_ARCHON = 23469;
	private static final int UNWORDLY_ARCHON = 23475;
	private static final int ABYSSAL_ARCHON = 23482;
	
	private static final NpcStringId[] ARCHON_MSG =
	{
		NpcStringId.DO_YOU_KNOW_WHO_IT_IS_THAT_YOU_FACE_IT_IS_BEST_THAT_YOU_RUN_NOW,
		NpcStringId.LOOK_INTO_MY_EYES_AND_SEE_WHAT_COUNTLESS_OTHERS_HAVE_SEEN_BEFORE_THEIR_DEATH,
		NpcStringId.I_COMMEND_YOUR_TENACITY_IN_COMING_THIS_FAR_BUT_NOW_IT_ENDS,
		NpcStringId.DO_YOU_SEE_THIS_SWORD_THE_LIGHT_THAT_SCREAMS_WITH_THE_LIFE_OF_THOSE_IT_HAS_KILLED,
		NpcStringId.I_WILL_SHOW_YOU_WHAT_TRUE_POWER_IS,
		NpcStringId.YOU_WILL_NEED_TO_SURPASS_ME_EVENTUALLY_BUT_DON_T_FORGET_KNOWING_HOW_TO_FLEE_IS_AN_IMPORTANT_PART_OF_BATTLE
	};
	
	public DimensionalArchon()
	{
		super();
		addSpawnId(DEMINSIONAL_ARCHON, UNWORDLY_ARCHON, ABYSSAL_ARCHON);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if (event.equals("NPC_SHOUT"))
		{
			npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), getRandomEntry(ARCHON_MSG)));
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.setShowSummonAnimation(true);
		startQuestTimer("NPC_SHOUT", 2000, npc, null);
	}
	
	public static void main(String[] args)
	{
		new DimensionalArchon();
	}
}
