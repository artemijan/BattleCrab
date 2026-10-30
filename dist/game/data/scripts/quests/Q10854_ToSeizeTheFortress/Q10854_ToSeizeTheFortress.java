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
package quests.Q10854_ToSeizeTheFortress;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * To Seize the Fortress (10854)
 * @author Kazumi
 */
public final class Q10854_ToSeizeTheFortress extends Quest
{
	// NPCs
	private static final int VAN_DYKE = 34235;
	private static final int BARTON = 34059;
	private static final int HAYUK = 34060;
	private static final int ELISE = 34061;
	private static final int ELIYAH = 34062;
	
	// Mobs
	private static final int GEORK = 23586;
	private static final int QUARTERMASTER = 23588;
	private static final int BURNSTEIN = 26136;
	
	// Items
	private static final int REPORT_BARTON = 47193;
	private static final int REPORT_HAYUK = 47194;
	private static final int REPORT_ELISE = 47195;
	private static final int REPORT_ELIYAH = 47196;
	private static final int RUNE_STONE = 39738;
	private static final int SPELLBOOK_WAR_HORSE = 47149;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	private static boolean _bartonReport = false;
	private static boolean _hayukReport = false;
	private static boolean _eliseReport = false;
	private static boolean _eliyahReport = false;
	
	public Q10854_ToSeizeTheFortress()
	{
		super(10854);
		addStartNpc(VAN_DYKE);
		addTalkId(VAN_DYKE, BARTON, HAYUK, ELISE, ELIYAH);
		addKillId(GEORK, QUARTERMASTER);
		addCondMinLevel(MIN_LEVEL, "royal_maestre_q10854_02.htm");
		addFactionLevel(Faction.KINGDOM_ROYAL_GUARDS, 10, "royal_maestre_q10854_03.htm");
		registerQuestItems(REPORT_BARTON, REPORT_HAYUK, REPORT_ELISE, REPORT_ELIYAH);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		String htmltext = event;
		if (qs == null)
		{
			return null;
		}
		
		if (event.equals("quest_accept"))
		{
			htmltext = "royal_maestre_q10854_06.htm";
			qs.startQuest();
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
				if (npc.getId() == VAN_DYKE)
				{
					if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 10)
					{
						if (player.getLevel() >= MIN_LEVEL)
						{
							htmltext = "royal_maestre_q10854_01.htm";
							break;
						}
						
						htmltext = "royal_maestre_q10854_02.htm";
						break;
					}
					
					htmltext = "royal_maestre_q10854_03.htm";
					break;
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case VAN_DYKE:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								if (hasQuestItems(player, REPORT_BARTON) && hasQuestItems(player, REPORT_HAYUK) && hasQuestItems(player, REPORT_ELISE) && hasQuestItems(player, REPORT_ELIYAH))
								{
									qs.setCond(2);
									htmltext = "royal_maestre_q10854_08.htm";
									break;
								}
								
								htmltext = "royal_maestre_q10854_07.htm";
								break;
							}
							case 2:
							{
								htmltext = "royal_maestre_q10854_08.htm";
								break;
							}
							case 3:
							{
								qs.setCond(4);
								htmltext = "royal_maestre_q10854_09.htm";
								break;
							}
							case 4:
							{
								htmltext = "royal_maestre_q10854_12.htm";
								break;
							}
							case 5:
							{
								htmltext = "royal_maestre_q10854_10.htm";
								break;
							}
						}
						break;
					}
					case BARTON:
					{
						if (qs.isCond(1))
						{
							if (hasQuestItems(player, REPORT_BARTON))
							{
								htmltext = "member_barton_q10854_04.htm";
								break;
							}
							
							if (!_bartonReport)
							{
								giveItems(player, REPORT_BARTON, 1);
								htmltext = "member_barton_q10854_03.htm";
								break;
							}
							
							htmltext = "member_barton_q10854_01.htm";
						}
						break;
					}
					case HAYUK:
					{
						if (qs.isCond(1))
						{
							if (hasQuestItems(player, REPORT_HAYUK))
							{
								htmltext = "member_hayuk_q10854_04.htm";
								break;
							}
							
							if (!_hayukReport)
							{
								giveItems(player, REPORT_HAYUK, 1);
								htmltext = "member_hayuk_q10854_03.htm";
								break;
							}
							
							htmltext = "member_hayuk_q10854_01.htm";
						}
						break;
					}
					case ELISE:
					{
						if (qs.isCond(1))
						{
							if (hasQuestItems(player, REPORT_ELISE))
							{
								htmltext = "member_alice_q10854_04.htm";
								break;
							}
							
							if (!_eliseReport)
							{
								giveItems(player, REPORT_ELISE, 1);
								htmltext = "member_alice_q10854_03.htm";
								break;
							}
							
							htmltext = "member_alice_q10854_01.htm";
						}
						break;
					}
					case ELIYAH:
					{
						if (qs.isCond(1))
						{
							if (hasQuestItems(player, REPORT_ELIYAH))
							{
								htmltext = "member_elliyah_q10854_04.htm";
								break;
							}
							
							if (!_eliyahReport)
							{
								giveItems(player, REPORT_ELIYAH, 1);
								htmltext = "member_elliyah_q10854_03.htm";
								break;
							}
							
							htmltext = "member_elliyah_q10854_01.htm";
						}
						break;
					}
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
	@Id(VAN_DYKE)
	@Id(BARTON)
	@Id(HAYUK)
	@Id(ELISE)
	@Id(ELIYAH)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10854)
		{
			switch (reply)
			{
				case 1:
				{
					switch (npc.getId())
					{
						case VAN_DYKE:
						{
							showHtmlFile(player, "royal_maestre_q10854_04.htm", npc);
							break;
						}
						case BARTON:
						{
							giveItems(player, REPORT_BARTON, 1);
							_bartonReport = true;
							showHtmlFile(player, "member_barton_q10854_02.htm", npc);
							break;
						}
						case HAYUK:
						{
							giveItems(player, REPORT_HAYUK, 1);
							_hayukReport = true;
							showHtmlFile(player, "member_hayuk_q10854_02.htm", npc);
							break;
						}
						case ELISE:
						{
							giveItems(player, REPORT_ELISE, 1);
							_eliseReport = true;
							showHtmlFile(player, "member_alice_q10854_02.htm", npc);
							break;
						}
						case ELIYAH:
						{
							giveItems(player, REPORT_ELIYAH, 1);
							_eliyahReport = true;
							showHtmlFile(player, "member_elliyah_q10854_02.htm", npc);
							break;
						}
					}
					break;
				}
				case 2:
				{
					showHtmlFile(player, "royal_maestre_q10854_05.htm", npc);
					break;
				}
				case 10:
				{
					qs.exitQuest(false, true);
					addExpAndSp(player, 203344446600L, 488025000);
					giveItems(player, RUNE_STONE, 1);
					giveItems(player, SPELLBOOK_WAR_HORSE, 1);
					showHtmlFile(player, "royal_maestre_q10854_11.htm", npc);
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isStarted())
		{
			if (qs.isCond(2))
			{
				int killedGeork = qs.getInt("killed_" + GEORK);
				int killedHummel = qs.getInt("killed_" + QUARTERMASTER);
				switch (npc.getId())
				{
					case GEORK:
					{
						if (killedGeork < 1)
						{
							qs.set("killed_" + GEORK, 1);
							playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						}
						
						sendNpcLogList(player);
						break;
					}
					case QUARTERMASTER:
					{
						if (killedHummel < 1)
						{
							qs.set("killed_" + GEORK, 1);
							playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						}
						
						sendNpcLogList(player);
						break;
					}
				}
				
				if ((killedGeork == 1) && (killedHummel == 1))
				{
					qs.setCond(3, true);
				}
			}
			else if (qs.isCond(4))
			{
				if (npc.getId() == BURNSTEIN)
				{
					qs.setCond(5, true);
				}
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(2))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(GEORK, false, qs.getInt("killed_" + GEORK)));
			holder.add(new NpcLogListHolder(QUARTERMASTER, false, qs.getInt("killed_" + QUARTERMASTER)));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		sendNpcLogList(player);
	}
}
