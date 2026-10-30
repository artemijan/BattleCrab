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
package ai.others.BalthusKnights.Monsters.GemDragon;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureDamageReceived;
import org.l2jmobius.gameserver.model.events.returns.DamageReturn;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import instances.BalthusKnights.AntharasNest;
import quests.Q10553_WhatMattersMoreThanAbility.Q10553_WhatMattersMoreThanAbility;
import quests.Q10555_ChargeAtAntharas.Q10555_ChargeAtAntharas;

/**
 * Gem Dragon AI
 * @author Kazumi
 */
public final class GemDragon extends Script
{
	// NPCs
	private static final int STIG_MACH_FRIEND = 34366;
	
	// Monsters
	private static final int GEM_DRAGON = 24097;
	private static final int GEM_DRAGON_ANTHARAS = 24091;
	
	// Skills
	private static final double DAMAGE_BY_SKILL = 0.5d;
	private static final SkillHolder BALTHUS_KNIGHT_YOKE = new SkillHolder(32167, 1);
	private static final SkillHolder BALTHUS_KNIGHT_YOKE_RELEASE = new SkillHolder(32168, 1);
	
	public GemDragon()
	{
		addAttackId(GEM_DRAGON);
		addSpawnId(GEM_DRAGON, GEM_DRAGON_ANTHARAS);
		addKillId(GEM_DRAGON_ANTHARAS);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon, Skill skill)
	{
		if ((skill == null) && npc.isAffectedBySkill(BALTHUS_KNIGHT_YOKE))
		{
			showOnScreenMsg(attacker, NpcStringId.USE_A_SKILL_ON_THE_GEM_DRAGON, ExShowScreenMessage.TOP_CENTER, 5000, false);
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if (npc.getId() == GEM_DRAGON_ANTHARAS)
		{
			final QuestState qs = killer.getQuestState(Q10555_ChargeAtAntharas.class.getSimpleName());
			if ((qs != null) && qs.isCond(2))
			{
				final Quest instance = ScriptManager.getInstance().getScript(AntharasNest.class.getSimpleName());
				if (instance != null)
				{
					ThreadPool.schedule(() ->
					{
						instance.onEvent("continueGemDragonsAttack", npc, killer);
					}, 4000); // 4 sec
					
				}
			}
		}
	}
	
	@RegisterEvent(EventType.ON_CREATURE_DAMAGE_RECEIVED)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(GEM_DRAGON)
	public DamageReturn onCreatureDamageReceived(OnCreatureDamageReceived event)
	{
		final Creature target = event.getTarget();
		if (target.isNpc() && (event.getAttacker().isPlayer() || event.getAttacker().isSummon()))
		{
			final Player player = event.getAttacker().asPlayer();
			final Instance instance = player.getInstanceWorld();
			if ((instance != null) && (instance.getTemplateId() == 270))
			{
				final QuestState qs = player.getQuestState(Q10553_WhatMattersMoreThanAbility.class.getSimpleName());
				if ((qs != null) && qs.isCond(3))
				{
					if (event.getSkill() != null)
					{
						if (target.isAffectedBySkill(BALTHUS_KNIGHT_YOKE))
						{
							target.stopSkillEffects(BALTHUS_KNIGHT_YOKE.getSkill());
							target.doCast(BALTHUS_KNIGHT_YOKE_RELEASE.getSkill());
							return new DamageReturn(false, true, false, target.getMaxHp() * DAMAGE_BY_SKILL);
						}
						
						return new DamageReturn(false, true, false, target.getMaxHp() * DAMAGE_BY_SKILL);
					}
					
					if (target.isAffectedBySkill(BALTHUS_KNIGHT_YOKE))
					{
						return new DamageReturn(true, true, true, 0);
					}
				}
			}
			
			return null;
		}
		
		return null;
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		switch (npc.getId())
		{
			case GEM_DRAGON:
			{
				if ((instance != null) && (instance.getTemplateId() == 270))
				{
					npc.doCast(BALTHUS_KNIGHT_YOKE.getSkill());
					npc.setRandomWalking(false);
				}
				break;
			}
			case GEM_DRAGON_ANTHARAS:
			{
				if ((instance != null) && (instance.getTemplateId() == 271) && (instance.getStatus() == 1))
				{
					final Npc stig = instance.getNpc(STIG_MACH_FRIEND);
					npc.asAttackable().addDamageHate(stig, 1, 99999);
					addAttackDesire(npc, stig);
				}
				break;
			}
		}
	}
	
	public static void main(String[] args)
	{
		new GemDragon();
	}
}
