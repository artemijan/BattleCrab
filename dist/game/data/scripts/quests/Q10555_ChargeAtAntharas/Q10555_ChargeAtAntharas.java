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
package quests.Q10555_ChargeAtAntharas;

import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.variables.PlayerVariables;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;
import org.l2jmobius.gameserver.network.serverpackets.ServerPacket;

import instances.BalthusKnights.AntharasNest;
import quests.Q10554_GiftForYou.Q10554_GiftForYou;

/**
 * Charge at Antharas (10555)
 * @author Kazumi
 */
public final class Q10555_ChargeAtAntharas extends Quest
{
	// NPCs
	private static final int STIG = 34361;
	private static final int STIG_MACH_FRIEND = 34366;
	private static final int ROIEN = 34373;
	private static final int BROME = 34374;
	private static final int MION = 34375;
	private static final int TAKHUN = 34376;
	private static final int TARIAH = 34377;
	private static final int ELIAS = 34378;
	private static final int ELENA = 34379;
	private static final int ULTRIAN = 34380;
	private static final int NARITA = 34381;
	private static final int KAYLEEN = 34382;
	
	// Monsters
	private static final int GEM_DRAGON_ANTHARAS = 24091;
	
	public Q10555_ChargeAtAntharas()
	{
		super(10555);
		addStartNpc(STIG);
		addTalkId(STIG);
		addKillId(GEM_DRAGON_ANTHARAS);
		addCondCompletedQuest(Q10554_GiftForYou.class.getSimpleName(), "stig_q10555_02.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		if (event.equals("quest_accept"))
		{
			qs.startQuest();
			htmltext = "stig_q10555_05.htm";
			showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 10000, true);
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "stig_q10555_01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.getCond() < 16)
				{
					htmltext = "stig_q10555_06.htm";
				}
				else
				{
					qs.exitQuest(false, true);
					player.addExpAndSp(860_081_566L, 300_000L);
					final ServerPacket packet = new ExTutorialShowId(18);
					player.sendPacket(packet);
					htmltext = "stig_q10555_07.htm";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(STIG)
	@Id(STIG_MACH_FRIEND)
	@Id(ROIEN)
	@Id(BROME)
	@Id(MION)
	@Id(TAKHUN)
	@Id(TARIAH)
	@Id(ELIAS)
	@Id(ELENA)
	@Id(ULTRIAN)
	@Id(NARITA)
	@Id(KAYLEEN)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10555)
		{
			switch (reply)
			{
				case 1:
				{
					switch (npc.getId())
					{
						case STIG_MACH_FRIEND:
						{
							final Quest instance = ScriptManager.getInstance().getScript(AntharasNest.class.getSimpleName());
							if (instance != null)
							{
								instance.onEvent("startGemDragonsAttack", npc, player);
							}
							
							qs.setCond(2, true);
							player.getVariables().set(PlayerVariables.BALTHUS_PHASE, 3);
							showHtmlFile(player, "stig_friend002.htm");
							break;
						}
						case STIG:
						{
							showHtmlFile(player, "stig_q10555_03.htm");
							break;
						}
						case ROIEN:
						{
							qs.setCond(14, false);
							giveItems(player, 48198, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.roien_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_CUTTER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case BROME:
						{
							qs.setCond(14, false);
							giveItems(player, 48195, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.brome_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_SLASHER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case MION:
						{
							qs.setCond(14, false);
							giveItems(player, 48194, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.mion_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_SHAPER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case TAKHUN:
						{
							qs.setCond(14, false);
							giveItems(player, 48201, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.takun_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_DUALSWORD, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case TARIAH:
						{
							qs.setCond(14, false);
							giveItems(player, 48200, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.taria_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_FIGHTER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case ELIAS:
						{
							qs.setCond(14, false);
							giveItems(player, 48197, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.elias_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_BUSTER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case ELENA:
						{
							qs.setCond(14, false);
							giveItems(player, 48197, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.elena_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_BUSTER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case ULTRIAN:
						{
							qs.setCond(14, false);
							giveItems(player, 48199, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.oltrian_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_STORMER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case NARITA:
						{
							qs.setCond(14, false);
							giveItems(player, 48199, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.narita_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_STORMER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
						case KAYLEEN:
						{
							qs.setCond(14, false);
							giveItems(player, 48196, 1);
							final Instance instance = npc.getInstanceWorld();
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.cairin2_ep50_battle_5", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WEAPON_WILL_GIVE_YOU_STRENGTH);
							showOnScreenMsg(player, NpcStringId.EQUIP_AN_ANTHARAS_THROWER, ExShowScreenMessage.TOP_CENTER, 5000, true);
							getTimers().addTimer("despawn_me", 5000L, n ->
							{
								npc.decayMe();
								showOnScreenMsg(player, NpcStringId.USE_THE_ITEM_SKILL_ANTHARAS_BREATH_ON_ANTHARAS, ExShowScreenMessage.TOP_CENTER, 5000, true);
							});
							break;
						}
					}
					break;
				}
				case 2:
				{
					showHtmlFile(player, "stig_q10555_04.htm");
					break;
				}
				case 3:
				{
					final Quest instance = ScriptManager.getInstance().getScript(AntharasNest.class.getSimpleName());
					if (instance != null)
					{
						instance.onEvent("enterInstance", npc, player);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if (qs != null)
		{
			if ((npc.getId() == GEM_DRAGON_ANTHARAS) && qs.isCond(2))
			{
				int dragonKills = qs.getInt(Integer.toString(GEM_DRAGON_ANTHARAS));
				if (dragonKills < GEM_DRAGON_ANTHARAS)
				{
					dragonKills++;
					qs.set(Integer.toString(GEM_DRAGON_ANTHARAS), dragonKills);
				}
				
				final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
				log.addNpc(GEM_DRAGON_ANTHARAS, qs.getInt(Integer.toString(GEM_DRAGON_ANTHARAS)));
				qs.getPlayer().sendPacket(log);
				
				if (qs.getInt(Integer.toString(GEM_DRAGON_ANTHARAS)) >= 5)
				{
					qs.setCond(3, false);
					final Quest instance = ScriptManager.getInstance().getScript(AntharasNest.class.getSimpleName());
					if (instance != null)
					{
						instance.onEvent("startAntharasProgress", npc, killer);
					}
				}
			}
		}
	}
}
