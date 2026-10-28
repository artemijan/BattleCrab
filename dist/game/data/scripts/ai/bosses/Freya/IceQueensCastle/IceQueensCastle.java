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
package ai.bosses.Freya.IceQueensCastle;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.enums.Movie;

import quests.Q10285_MeetingSirra.Q10285_MeetingSirra;

/**
 * Ice Queen's Castle instance zone.
 * @author Adry_85
 */
public class IceQueensCastle extends InstanceScript
{
	// NPCs
	private static final int FREYA = 18847;
	private static final int BATTALION_LEADER = 18848;
	private static final int LEGIONNAIRE = 18849;
	private static final int MERCENARY_ARCHER = 18926;
	private static final int ARCHERY_KNIGHT = 22767;
	private static final int JINIA = 32781;
	
	// Locations
	private static final Location FREYA_LOC = new Location(114730, -114805, -11200, 50);
	
	// Skill
	private static final SkillHolder ETHERNAL_BLIZZARD = new SkillHolder(6276, 1);
	
	// Misc
	private static final int TEMPLATE_ID = 137;
	
	public IceQueensCastle()
	{
		super(TEMPLATE_ID);
		addStartNpc(JINIA);
		addTalkId(JINIA);
		addSpawnId(FREYA);
		addSpellFinishedId(FREYA);
		addCreatureSeeId(BATTALION_LEADER, LEGIONNAIRE, MERCENARY_ARCHER);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "ATTACK_KNIGHT":
			{
				World.getInstance().forEachVisibleObject(npc, Npc.class, mob ->
				{
					if ((mob.getId() == ARCHERY_KNIGHT) && !mob.isDead() && !mob.isDecayed())
					{
						npc.setRunning();
						npc.getAI().setIntention(Intention.ATTACK, mob);
						npc.asAttackable().addDamageHate(mob, 0, 999999);
					}
				});
				startQuestTimer("ATTACK_KNIGHT", 3000, npc, null);
				break;
			}
			case "TIMER_MOVING":
			{
				if (npc != null)
				{
					npc.getAI().setIntention(Intention.MOVE_TO, FREYA_LOC);
				}
				break;
			}
			case "TIMER_BLIZZARD":
			{
				npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_CAN_NO_LONGER_STAND_BY);
				npc.stopMove(null);
				npc.setTarget(player);
				npc.doCast(ETHERNAL_BLIZZARD.getSkill());
				break;
			}
			case "TIMER_SCENE_21":
			{
				if (npc != null)
				{
					playMovie(player, Movie.SC_BOSS_FREYA_FORCED_DEFEAT);
					startQuestTimer("TIMER_PC_LEAVE", 24000, null, player);
					npc.deleteMe();
				}
				break;
			}
			case "TIMER_PC_LEAVE":
			{
				final QuestState qs = player.getQuestState(Q10285_MeetingSirra.class.getSimpleName());
				if (qs != null)
				{
					qs.setMemoState(3);
					qs.setCond(10, true);
					finishInstance(player, 0);
				}
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		enterInstance(talker, npc, TEMPLATE_ID);
		return super.onTalk(npc, talker);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("TIMER_MOVING", 60000, npc, null);
		startQuestTimer("TIMER_BLIZZARD", 180000, npc, null);
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer() && npc.isScriptValue(0))
		{
			npc.setScriptValue(1);
			npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.S1_MAY_THE_PROTECTION_OF_THE_GODS_BE_UPON_YOU, creature.getName());
			World.getInstance().forEachVisibleObject(npc, Npc.class, mob ->
			{
				if ((mob.getId() == ARCHERY_KNIGHT) && !mob.isDead() && !mob.isDecayed())
				{
					npc.setRunning();
					npc.getAI().setIntention(Intention.ATTACK, mob);
					npc.asAttackable().addDamageHate(mob, 0, 999999);
				}
			});
			startQuestTimer("ATTACK_KNIGHT", 5000, npc, null);
		}
	}
	
	@Override
	public void onSpellFinished(Npc npc, Player player, Skill skill)
	{
		final Instance world = npc.getInstanceWorld();
		if ((world != null) && (skill == ETHERNAL_BLIZZARD.getSkill()))
		{
			final Player playerInside = world.getFirstPlayer();
			if (playerInside != null)
			{
				startQuestTimer("TIMER_SCENE_21", 1000, npc, playerInside);
			}
		}
	}
	
	public static void main(String[] args)
	{
		new IceQueensCastle();
	}
}
