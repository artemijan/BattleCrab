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

import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * Dimensional Trap's AI
 * @author Gigi
 * @date 2018-09-07 - [17:39:04]
 */
public class DimensionalTrap extends Script
{
	private static final int DIMENSIONAL_DEBUFF_TRAP_1 = 19556; // Debuff trap, power 1
	private static final int DIMENSIONAL_DEBUFF_TRAP_2 = 19557; // Debuff trap, power 2
	private static final int DIMENSIONAL_DEBUFF_TRAP_3 = 19558; // Debuff trap, power 3
	private static final int DIMENSIONAL_DEMAGE_TRAP_1 = 19559; // Damage trap, power 1
	private static final int DIMENSIONAL_DEMAGE_TRAP_2 = 19560; // Damage trap, power 2
	private static final int DIMENSIONAL_DEMAGE_TRAP_3 = 19561; // Damage trap, power 3
	private static final int DIMENSIONAL_HEAL_LIGHT = 19562; // Heal Trap
	
	// skill
	private static final SkillHolder TRAP_HOLD = new SkillHolder(16409, 1);
	private static final SkillHolder TRAP_ARIALL_YOKE = new SkillHolder(16410, 1);
	private static final SkillHolder TRAP_STUN = new SkillHolder(16411, 1);
	private static final SkillHolder TRAP_POYSON = new SkillHolder(16413, 1);
	private static final SkillHolder PEACE_ZONE_CURE = new SkillHolder(16414, 1);
	
	private int _type;
	
	public DimensionalTrap()
	{
		super();
		addSpawnId(DIMENSIONAL_DEBUFF_TRAP_1, DIMENSIONAL_DEBUFF_TRAP_2, DIMENSIONAL_DEBUFF_TRAP_3, DIMENSIONAL_DEMAGE_TRAP_1, DIMENSIONAL_DEMAGE_TRAP_2, DIMENSIONAL_DEMAGE_TRAP_3, DIMENSIONAL_HEAL_LIGHT);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if ((npc == null) || npc.isAlikeDead())
		{
			return null;
		}
		
		switch (event)
		{
			case "debuff_player":
			{
				World.getInstance().forEachVisibleObjectInRange(npc, Player.class, _type, p ->
				{
					if ((p != null) && p.isPlayer() && !p.isDead())
					{
						npc.setTarget(p);
						npc.doCast((getRandom(10) < 5) ? TRAP_HOLD.getSkill() : TRAP_ARIALL_YOKE.getSkill());
					}
				});
				startQuestTimer("debuff_player", 10000, npc, null);
				break;
			}
			case "damage_player":
			{
				World.getInstance().forEachVisibleObjectInRange(npc, Player.class, _type, p ->
				{
					if ((p != null) && p.isPlayer() && !p.isDead())
					{
						npc.setTarget(p);
						npc.doCast((getRandom(10) < 5) ? TRAP_STUN.getSkill() : TRAP_POYSON.getSkill());
					}
				});
				startQuestTimer("damage_player", 10000, npc, null);
				break;
			}
			case "heal_player":
			{
				World.getInstance().forEachVisibleObjectInRange(npc, Player.class, _type, p ->
				{
					if ((p != null) && p.isPlayer() && !p.isDead())
					{
						npc.setTarget(p);
						npc.doCast(PEACE_ZONE_CURE.getSkill());
					}
				});
				startQuestTimer("heal_player", 10000, npc, null);
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		npc.setInvul(true);
		switch (npc.getId())
		{
			case DIMENSIONAL_DEBUFF_TRAP_1:
			{
				npc.setDisplayEffect(1);
				startQuestTimer("debuff_player", 3000, npc, null);
				_type = 50;
				break;
			}
			case DIMENSIONAL_DEBUFF_TRAP_2:
			{
				npc.setDisplayEffect(2);
				startQuestTimer("debuff_player", 3000, npc, null);
				_type = 100;
				break;
			}
			case DIMENSIONAL_DEBUFF_TRAP_3:
			{
				npc.setDisplayEffect(3);
				startQuestTimer("debuff_player", 3000, npc, null);
				_type = 150;
				break;
			}
			case DIMENSIONAL_DEMAGE_TRAP_1:
			{
				npc.setDisplayEffect(4);
				startQuestTimer("damage_player", 3000, npc, null);
				_type = 50;
				break;
			}
			case DIMENSIONAL_DEMAGE_TRAP_2:
			{
				npc.setDisplayEffect(5);
				startQuestTimer("damage_player", 3000, npc, null);
				_type = 100;
				break;
			}
			case DIMENSIONAL_DEMAGE_TRAP_3:
			{
				npc.setDisplayEffect(6);
				startQuestTimer("damage_player", 3000, npc, null);
				_type = 150;
				break;
			}
			case DIMENSIONAL_HEAL_LIGHT:
			{
				npc.setDisplayEffect(7);
				startQuestTimer("heal_player", 3000, npc, null);
				_type = 150;
				break;
			}
		}
	}
	
	public static void main(String[] args)
	{
		new DimensionalTrap();
	}
}
