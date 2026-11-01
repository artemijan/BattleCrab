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
package instances.FaeronTrainingGrounds2;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;

import quests.Q10736_ASpecialPower.Q10736_ASpecialPower;

/**
 * Fearon Training Grounds Instance Zone.
 * @author Sdw, malyelfik, Mobius
 */
public class FaeronTrainingGrounds2 extends InstanceScript
{
	// NPCs
	private static final int KATALIN_2 = 33945;
	
	// Monsters
	private static final int FLOATO = 27526;
	private static final int FLOATO2 = 27531;
	private static final int RATEL = 27527;
	
	// Misc
	private static final int TEMPLATE_ID = 252;
	
	// Rewards
	private static final int EXP_REWARD = 1716;
	private static final int SP_REWARD = 0;
	private static final int NG_SOULSHOTS_REWARD = 1835;
	
	public FaeronTrainingGrounds2()
	{
		super(TEMPLATE_ID);
		addFirstTalkId(KATALIN_2);
		addTalkId(KATALIN_2);
		addKillId(FLOATO, FLOATO2, RATEL);
		addAttackId(RATEL);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = player.getQuestState(Q10736_ASpecialPower.class.getSimpleName());
		final Instance world = player.getInstanceWorld();
		String htmltext = null;
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "enter_instance":
			{
				enterInstance(player, npc, TEMPLATE_ID);
				break;
			}
			case "exit_instance":
			{
				finishInstance(player, 0);
				break;
			}
			case "33945-03.html":
			{
				if (qs.isCond(6))
				{
					showOnScreenMsg(player, NpcStringId.FIGHT_USING_SKILLS, ExShowScreenMessage.TOP_CENTER, 10000);
				}
				else
				{
					showOnScreenMsg(player, NpcStringId.ATTACK_THE_MONSTER, ExShowScreenMessage.TOP_CENTER, 10000);
				}
				
				htmltext = event;
				break;
			}
			case "33945-07.html":
			{
				if (qs.isCond(5))
				{
					qs.setCond(6, true);
					showOnScreenMsg(player, NpcStringId.FIGHT_USING_SKILLS, ExShowScreenMessage.TOP_CENTER, 10000);
					world.spawnGroup("Ratel");
					htmltext = event;
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = player.getQuestState(Q10736_ASpecialPower.class.getSimpleName());
		final Instance world = player.getInstanceWorld();
		String htmltext = getNoQuestMsg(player);
		if (qs == null)
		{
			return htmltext;
		}
		
		if (qs.isStarted())
		{
			switch (qs.getCond())
			{
				case 1:
				{
					qs.setCond(2, true);
					world.spawnGroup("Floato");
					htmltext = "33945-01.html";
					break;
				}
				case 2:
				case 4:
				case 6:
				{
					htmltext = "33945-02.html";
					break;
				}
				case 3:
				{
					if (qs.getInt("shots") == 1)
					{
						qs.setCond(4, true);
						world.spawnGroup("Floato2");
						htmltext = "33945-05.html";
					}
					else
					{
						qs.set("shots", 1);
						giveItems(player, NG_SOULSHOTS_REWARD, 150);
						showOnScreenMsg(player, NpcStringId.AUTOMATE_SOULSHOT_AS_SHOWN_IN_THE_TUTORIAL, ExShowScreenMessage.TOP_CENTER, 10000);
						player.sendPacket(new ExTutorialShowId(14));
						htmltext = "33945-04.html";
						if (player.getLevel() < 5)
						{
							addExpAndSp(player, EXP_REWARD, SP_REWARD);
						}
					}
					break;
				}
				case 5:
				{
					player.sendPacket(new ExTutorialShowId(15));
					htmltext = "33945-06.html";
					break;
				}
				case 7:
				{
					htmltext = "33945-08.html";
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onAttack(Npc npc, Player player, int damage, boolean isSummon, Skill skill)
	{
		if (skill != null)
		{
			if (!npc.isDead() && (npc.getId() == RATEL))
			{
				final double dmg = npc.getMaxHp() * 0.5;
				npc.reduceCurrentHp(dmg, player, null);
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (world == null)
		{
			return;
		}
		
		final QuestState qs = player.getQuestState(Q10736_ASpecialPower.class.getSimpleName());
		if (qs != null)
		{
			switch (npc.getId())
			{
				case FLOATO:
				{
					onKillStateChange(player, qs);
					break;
				}
				case FLOATO2:
				{
					onKillStateChange(player, qs);
					break;
				}
				case RATEL:
				{
					onKillStateChange(player, qs);
					showOnScreenMsg(player, NpcStringId.TALK_TO_KATALIN_TO_LEAVE_THE_TRAINING_GROUNDS, ExShowScreenMessage.TOP_CENTER, 10000);
					break;
				}
			}
		}
	}
	
	private void onKillStateChange(Player player, QuestState qs)
	{
		int _killCount = qs.getMemoStateEx(Q10736_ASpecialPower.KILL_COUNT) + 1;
		if (_killCount >= 2)
		{
			qs.setCond(qs.getCond() + 1, true);
			playSound(player, QuestSound.ITEMSOUND_QUEST_MIDDLE);
			_killCount = 0;
		}
		playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
		qs.setMemoStateEx(Q10736_ASpecialPower.KILL_COUNT, _killCount);
		qs.getQuest().sendNpcLogList(player);
	}
	
	public static void main(String[] args)
	{
		new FaeronTrainingGrounds2();
	}
}
