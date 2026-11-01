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
package instances.BalthusKnights;

import java.util.List;

import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.Q10552_ChallengeBalthusKnight.Q10552_ChallengeBalthusKnight;

/**
 * Hatchling Nest instance zone.
 * @author Kazumi
 */
public final class HatchlingNest extends InstanceScript
{
	// MOBs
	private static final int HATCHLING = 24089;
	
	// Misc
	private static final int TEMPLATE_ID = 269;
	
	public HatchlingNest()
	{
		super(TEMPLATE_ID);
		addKillId(HATCHLING);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		if (event.equals("enterInstance"))
		{
			final QuestState qs = player.getQuestState(Q10552_ChallengeBalthusKnight.class.getSimpleName());
			if ((qs != null) && qs.isStarted())
			{
				enterInstance(player, npc, TEMPLATE_ID);
				player.addSkill(SkillData.getInstance().getSkill(239, 5), true); // Expertise S
				player.refreshExpertisePenalty();
			}
		}
		
		return htmltext;
	}
	
	@Override
	protected void onEnter(Player player, Instance instance, boolean firstEnter)
	{
		showOnScreenMsg(instance, NpcStringId.ATTACK_A_HATCHLING, ExShowScreenMessage.TOP_CENTER, 30000, false);
		super.onEnter(player, instance, firstEnter);
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Instance instance = player.getInstanceWorld();
		if ((instance != null) && (instance.getTemplateId() == TEMPLATE_ID))
		{
			if (instance.getAliveNpcs(HATCHLING).isEmpty())
			{
				moveMonsters(instance.spawnGroup("balthus_start_2523_03m1"));
				showOnScreenMsg(player, NpcStringId.ATTACK_A_HATCHLING, ExShowScreenMessage.TOP_CENTER, 30000, false);
			}
		}
	}
	
	private void moveMonsters(List<Npc> monsterList)
	{
		for (Npc monster : monsterList)
		{
			final Instance instance = monster.getInstanceWorld();
			if (monster.isAttackable() && (instance != null) && !monster.isDead())
			{
				monster.setRandomWalking(false);
				final Location loc = instance.getTemplateParameters().getLocation("middlePointRoom");
				final Location moveTo = new Location(loc.getX() + getRandom(-100, 100), loc.getY() + getRandom(-100, 100), loc.getZ());
				monster.setRunning();
				addMoveToDesire(monster, moveTo, 23);
				monster.asAttackable().setCanReturnToSpawnPoint(false);
			}
		}
	}
	
	public static void main(String[] args)
	{
		new HatchlingNest();
	}
}
