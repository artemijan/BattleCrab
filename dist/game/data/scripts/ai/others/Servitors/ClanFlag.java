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
package ai.others.Servitors;

import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.clan.Clan;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * @author Mobius
 */
public class ClanFlag extends Script
{
	// NPC
	private static final int CLAN_FLAG = 19269;
	
	// Skills
	private static final SkillHolder BUFF = new SkillHolder(15095, 1);
	private static final SkillHolder DEBUFF = new SkillHolder(15096, 1);
	
	private ClanFlag()
	{
		addSpawnId(CLAN_FLAG);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		getTimers().addTimer("END_OF_LIFE", 1800000, npc, null);
		getTimers().addTimer("SKILL_CAST", 1000, npc, null);
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		switch (event)
		{
			case "SKILL_CAST":
			{
				if (npc.getSummoner() != null)
				{
					final Clan summonerClan = npc.getSummoner().getClan();
					if (summonerClan != null)
					{
						World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 2000, target ->
						{
							if ((target != null) && !target.isDead() && GeoEngine.getInstance().canSeeTarget(npc, target))
							{
								final Clan targetClan = target.getClan();
								if (targetClan != null)
								{
									if (targetClan == summonerClan)
									{
										BUFF.getSkill().applyEffects(npc, target);
									}
									else if (targetClan.isAtWarWith(summonerClan))
									{
										DEBUFF.getSkill().applyEffects(npc, target);
									}
								}
							}
						});
						getTimers().addTimer("SKILL_CAST", 3000, npc, null);
						return;
					}
				}
				
				getTimers().addTimer("END_OF_LIFE", 100, npc, null);
				break;
			}
			case "END_OF_LIFE":
			{
				getTimers().cancelTimer("SKILL_CAST", npc, null);
				npc.deleteMe();
				break;
			}
		}
	}
	
	public static void main(String[] args)
	{
		new ClanFlag();
	}
}
