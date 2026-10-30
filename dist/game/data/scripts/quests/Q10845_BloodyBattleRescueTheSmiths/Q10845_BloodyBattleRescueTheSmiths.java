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
package quests.Q10845_BloodyBattleRescueTheSmiths;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

import quests.Q10844_BloodyBattleSeizingSupplies.Q10844_BloodyBattleSeizingSupplies;

/**
 * Bloody Battle - Rescue the Smiths (10845)
 * @author Kazumi
 */
public final class Q10845_BloodyBattleRescueTheSmiths extends Quest
{
	// NPCs
	private static final int HURAK = 34064;
	private static final int CAPTIVE_BLACKSMITH_KARROD = 34139;
	private static final int CAPTIVE_BLACKSMITH_BRONK = 34141;
	private static final int CAPTIVE_BLACKSMITH_AIOS = 34140;
	private static final int CAPTIVE_BLACKSMITH_BRUNON = 34142;
	private static final int CAPTIVE_BLACKSMITH_SUMARI = 34143;
	private static final int CAPTIVE_BLACKSMITH_TRAINEE_LANSIA = 34144;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	
	public Q10845_BloodyBattleRescueTheSmiths()
	{
		super(10845);
		addStartNpc(HURAK);
		addTalkId(HURAK);
		addCondMinLevel(MIN_LEVEL, "blackbird_hurak_q10845_02.htm");
		addFactionLevel(Faction.KINGDOM_ROYAL_GUARDS, 3, "blackbird_hurak_q10845_02.htm");
		addCondCompletedQuest(Q10844_BloodyBattleSeizingSupplies.class.getSimpleName(), "blackbird_hurak_q10845_02.htm");
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
			htmltext = "blackbird_hurak_q10845_05.htm";
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
				if (player.getFactionLevel(Faction.KINGDOM_ROYAL_GUARDS) >= 3)
				{
					htmltext = "blackbird_hurak_q10845_01.htm";
					break;
				}
				
				htmltext = "blackbird_hurak_q10845_02.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "blackbird_hurak_q10845_06.htm";
						break;
					}
					case 2:
					{
						htmltext = "blackbird_hurak_q10845_07.htm";
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
	@Id(HURAK)
	@Id(CAPTIVE_BLACKSMITH_KARROD)
	@Id(CAPTIVE_BLACKSMITH_BRONK)
	@Id(CAPTIVE_BLACKSMITH_AIOS)
	@Id(CAPTIVE_BLACKSMITH_BRUNON)
	@Id(CAPTIVE_BLACKSMITH_SUMARI)
	@Id(CAPTIVE_BLACKSMITH_TRAINEE_LANSIA)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 10845)
		{
			switch (reply)
			{
				case 1:
				{
					switch (npc.getId())
					{
						case HURAK:
						{
							showHtmlFile(player, "blackbird_hurak_q10845_03.htm", npc);
							break;
						}
						case CAPTIVE_BLACKSMITH_BRONK:
						{
							npc.doDie(player);
							addSpawn(34147, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Bronk
							break;
						}
						case CAPTIVE_BLACKSMITH_AIOS:
						{
							int resAios = qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_AIOS));
							if (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_AIOS)) < 1)
							{
								resAios++;
								qs.set(Integer.toString(CAPTIVE_BLACKSMITH_AIOS), resAios);
								playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							}
							
							final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
							log.addNpcString(NpcStringId.RESCUING_AIOS, qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_AIOS))); // Rescuing Aios
							qs.getPlayer().sendPacket(log);
							checkLogState(player);
							npc.doDie(player);
							addSpawn(34146, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Aios
							break;
						}
						case CAPTIVE_BLACKSMITH_BRUNON:
						{
							int resBrunon = qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_BRUNON));
							if (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_BRUNON)) < 1)
							{
								resBrunon++;
								qs.set(Integer.toString(CAPTIVE_BLACKSMITH_BRUNON), resBrunon);
								playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							}
							
							final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
							log.addNpcString(NpcStringId.RESCUING_BRUNON, qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_BRUNON))); // Rescuing Brunon
							qs.getPlayer().sendPacket(log);
							checkLogState(player);
							npc.doDie(player);
							addSpawn(34148, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Brunon
							break;
						}
						case CAPTIVE_BLACKSMITH_SUMARI:
						{
							int resSumari = qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_SUMARI));
							if (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_SUMARI)) < 1)
							{
								resSumari++;
								qs.set(Integer.toString(CAPTIVE_BLACKSMITH_SUMARI), resSumari);
								playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							}
							
							final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
							log.addNpcString(NpcStringId.RESCUING_SUMARI, qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_SUMARI))); // Rescuing Sumari
							qs.getPlayer().sendPacket(log);
							checkLogState(player);
							npc.doDie(player);
							addSpawn(34149, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Sumari
							break;
						}
						case CAPTIVE_BLACKSMITH_TRAINEE_LANSIA:
						{
							int resLansia = qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_TRAINEE_LANSIA));
							if (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_TRAINEE_LANSIA)) < 1)
							{
								resLansia++;
								qs.set(Integer.toString(CAPTIVE_BLACKSMITH_TRAINEE_LANSIA), resLansia);
								playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							}
							
							final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
							log.addNpcString(NpcStringId.RESCUING_LANSIA, qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_TRAINEE_LANSIA))); // Rescuing Lansia
							qs.getPlayer().sendPacket(log);
							checkLogState(player);
							npc.doDie(player);
							addSpawn(34150, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Lansia
							break;
						}
						case CAPTIVE_BLACKSMITH_KARROD:
						{
							int resKarrod = qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_KARROD));
							if (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_KARROD)) < 1)
							{
								resKarrod++;
								qs.set(Integer.toString(CAPTIVE_BLACKSMITH_KARROD), resKarrod);
								playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
							}
							
							final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
							log.addNpcString(NpcStringId.RESCUING_KARROD, qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_KARROD))); // Rescuing Karrod
							qs.getPlayer().sendPacket(log);
							checkLogState(player);
							npc.doDie(player);
							addSpawn(34145, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 180000, false, 0); // Spawn Freed Karrod
							break;
						}
					}
					break;
				}
				case 2:
				{
					showHtmlFile(player, "blackbird_hurak_q10845_04.htm", npc);
					break;
				}
				case 10:
				{
					if (qs.isCond(2) && (player.getLevel() >= MIN_LEVEL))
					{
						qs.exitQuest(false, true);
						addExpAndSp(player, 7262301690L, 17429400);
						showHtmlFile(player, "blackbird_hurak_q10845_08.htm", npc);
					}
					break;
				}
			}
		}
	}
	
	public void checkLogState(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_KARROD)) == 1) && (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_AIOS)) == 1) && (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_BRUNON)) == 1) && (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_SUMARI)) == 1) && (qs.getInt(Integer.toString(CAPTIVE_BLACKSMITH_TRAINEE_LANSIA)) == 1))
		{
			qs.setCond(2, true);
		}
	}
}
