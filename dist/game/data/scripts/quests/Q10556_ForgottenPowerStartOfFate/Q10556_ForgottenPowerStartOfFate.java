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
package quests.Q10556_ForgottenPowerStartOfFate;

import java.util.HashMap;
import java.util.Map;
import java.util.Map.Entry;

import org.l2jmobius.gameserver.data.xml.SkillTreeData;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
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
import org.l2jmobius.gameserver.model.skill.CommonSkill;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillLearn;
import org.l2jmobius.gameserver.model.variables.PlayerVariables;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.UserInfoType;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.MagicSkillUse;
import org.l2jmobius.gameserver.network.serverpackets.SocialAction;
import org.l2jmobius.gameserver.network.serverpackets.UserInfo;
import org.l2jmobius.gameserver.network.serverpackets.ability.ExAcquireAPSkillList;

import instances.BalthusKnights.PowerOfAbelius.PowerOfAbelius;
import instances.BalthusKnights.PowerOfCranigg.PowerOfCranigg;
import instances.BalthusKnights.PowerOfKashnaga.PowerOfKashnaga;
import instances.BalthusKnights.PowerOfLakcis.PowerOfLakcis;
import instances.BalthusKnights.PowerOfNaviarope.PowerOfNaviarope;
import instances.BalthusKnights.PowerOfRaister.PowerOfRaister;
import instances.BalthusKnights.PowerOfSapyros.PowerOfSapyros;
import instances.BalthusKnights.PowerOfSoltkreig.PowerOfSoltkreig;
import quests.Q10555_ChargeAtAntharas.Q10555_ChargeAtAntharas;

/**
 * Forgotten Power: Start of Fate (10556)
 * @author Kazumi
 */
public final class Q10556_ForgottenPowerStartOfFate extends Quest
{
	// NPCs
	private static final int STIG = 34361;
	private static final int MASTER_DEFENDER = 34386;
	private static final int MEELE_DAMAGE_DEALER = 34387;
	private static final int DAGGER_MASTER = 34388;
	private static final int BOW_MASTER = 34389;
	private static final int BUFF_MASTER = 34391;
	private static final int MASTER_WIZARD = 34390;
	private static final int MASTER_SUMMONER = 34392;
	private static final int MASTER_HEALER = 34393;
	
	// Items
	private static final int AGATHION_GRIFFIN = 37374;
	private static final int CHAOS_POMANDER = 37374;
	private static final int GRADUATION_GIFT = 46782;
	private static final int PAULINA_EQUIPMENT_SET_R = 46919;
	private static final int ABELIUS_POWER = 32264; // Sigel Knight
	private static final int SAPYROS_POWER = 32265; // Tyrr Warrior
	private static final int ASHAGEN_POWER = 32266; // Othell Rogue
	private static final int CRANIGG_POWER = 32267; // Yul Archer
	private static final int SOLTKREIG_POWER = 32268; // Feoh Wizard
	private static final int NAVIAROPE_POWER = 32269; // Wynn Summoner
	private static final int LEISTER_POWER = 32270; // Iss Enchanter
	private static final int LAKCIS_POWER = 32271; // Aeore Healer
	
	// Misc
	private static final Map<PlayerClass, Integer> AWAKE_POWER = new HashMap<>();
	static
	{
		AWAKE_POWER.put(PlayerClass.TYRR_DUELIST, SAPYROS_POWER);
		AWAKE_POWER.put(PlayerClass.TYRR_DREADNOUGHT, SAPYROS_POWER);
		AWAKE_POWER.put(PlayerClass.TYRR_TITAN, SAPYROS_POWER);
		AWAKE_POWER.put(PlayerClass.TYRR_GRAND_KHAVATARI, SAPYROS_POWER);
		AWAKE_POWER.put(PlayerClass.TYRR_MAESTRO, SAPYROS_POWER);
		AWAKE_POWER.put(PlayerClass.TYRR_DOOMBRINGER, SAPYROS_POWER);
		AWAKE_POWER.put(PlayerClass.SIGEL_PHOENIX_KNIGHT, ABELIUS_POWER);
		AWAKE_POWER.put(PlayerClass.SIGEL_HELL_KNIGHT, ABELIUS_POWER);
		AWAKE_POWER.put(PlayerClass.SIGEL_EVA_TEMPLAR, ABELIUS_POWER);
		AWAKE_POWER.put(PlayerClass.SIGEL_SHILLIEN_TEMPLAR, ABELIUS_POWER);
		AWAKE_POWER.put(PlayerClass.OTHELL_ADVENTURER, ASHAGEN_POWER);
		AWAKE_POWER.put(PlayerClass.OTHELL_WIND_RIDER, ASHAGEN_POWER);
		AWAKE_POWER.put(PlayerClass.OTHELL_GHOST_HUNTER, ASHAGEN_POWER);
		AWAKE_POWER.put(PlayerClass.OTHELL_FORTUNE_SEEKER, ASHAGEN_POWER);
		AWAKE_POWER.put(PlayerClass.YUL_SAGITTARIUS, CRANIGG_POWER);
		AWAKE_POWER.put(PlayerClass.YUL_MOONLIGHT_SENTINEL, CRANIGG_POWER);
		AWAKE_POWER.put(PlayerClass.YUL_GHOST_SENTINEL, CRANIGG_POWER);
		AWAKE_POWER.put(PlayerClass.YUL_TRICKSTER, CRANIGG_POWER);
		AWAKE_POWER.put(PlayerClass.FEOH_ARCHMAGE, SOLTKREIG_POWER);
		AWAKE_POWER.put(PlayerClass.FEOH_SOULTAKER, SOLTKREIG_POWER);
		AWAKE_POWER.put(PlayerClass.FEOH_MYSTIC_MUSE, SOLTKREIG_POWER);
		AWAKE_POWER.put(PlayerClass.FEOH_STORM_SCREAMER, SOLTKREIG_POWER);
		AWAKE_POWER.put(PlayerClass.FEOH_SOUL_HOUND, SOLTKREIG_POWER);
		AWAKE_POWER.put(PlayerClass.ISS_HIEROPHANT, LEISTER_POWER);
		AWAKE_POWER.put(PlayerClass.ISS_SWORD_MUSE, LEISTER_POWER);
		AWAKE_POWER.put(PlayerClass.ISS_SPECTRAL_DANCER, LEISTER_POWER);
		AWAKE_POWER.put(PlayerClass.ISS_DOMINATOR, LEISTER_POWER);
		AWAKE_POWER.put(PlayerClass.ISS_DOOMCRYER, LEISTER_POWER);
		AWAKE_POWER.put(PlayerClass.WYNN_ARCANA_LORD, NAVIAROPE_POWER);
		AWAKE_POWER.put(PlayerClass.WYNN_ELEMENTAL_MASTER, NAVIAROPE_POWER);
		AWAKE_POWER.put(PlayerClass.WYNN_SPECTRAL_MASTER, NAVIAROPE_POWER);
		AWAKE_POWER.put(PlayerClass.AEORE_CARDINAL, LAKCIS_POWER);
		AWAKE_POWER.put(PlayerClass.AEORE_EVA_SAINT, LAKCIS_POWER);
		AWAKE_POWER.put(PlayerClass.AEORE_SHILLIEN_SAINT, LAKCIS_POWER);
	}
	
	public Q10556_ForgottenPowerStartOfFate()
	{
		super(10556);
		addStartNpc(STIG);
		addTalkId(STIG);
		addCondNotRace(Race.ERTHEIA, "");
		addCondCompletedQuest(Q10555_ChargeAtAntharas.class.getSimpleName(), "stig_q10556_02.htm");
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
			switch (player.getPlayerClass())
			{
				case SIGEL_KNIGHT:
				{
					htmltext = "stig_q10556_04a.htm";
					break;
				}
				case TYRR_WARRIOR:
				{
					htmltext = "stig_q10556_04b.htm";
					break;
				}
				case OTHELL_ROGUE:
				{
					htmltext = "stig_q10556_04c.htm";
					break;
				}
				case YUL_ARCHER:
				{
					htmltext = "stig_q10556_04d.htm";
					break;
				}
				case WYNN_SUMMONER:
				{
					htmltext = "stig_q10556_04e.htm";
					break;
				}
				case ISS_ENCHANTER:
				{
					htmltext = "stig_q10556_04f.htm";
					break;
				}
				case FEOH_WIZARD:
				{
					htmltext = "stig_q10556_04g.htm";
					break;
				}
				case AEORE_HEALER:
				{
					htmltext = "stig_q10556_04h.htm";
					break;
				}
			}
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
				htmltext = "stig_q10556_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (player.getPlayerClass())
				{
					case SIGEL_KNIGHT:
					{
						htmltext = "stig_q10556_05a.htm";
						break;
					}
					case TYRR_WARRIOR:
					{
						htmltext = "stig_q10556_05b.htm";
						break;
					}
					case OTHELL_ROGUE:
					{
						htmltext = "stig_q10556_05c.htm";
						break;
					}
					case YUL_ARCHER:
					{
						htmltext = "stig_q10556_05d.htm";
						break;
					}
					case WYNN_SUMMONER:
					{
						htmltext = "stig_q10556_05e.htm";
						break;
					}
					case ISS_ENCHANTER:
					{
						htmltext = "stig_q10556_05f.htm";
						break;
					}
					case FEOH_WIZARD:
					{
						htmltext = "stig_q10556_05g.htm";
						break;
					}
					case AEORE_HEALER:
					{
						htmltext = "stig_q10556_05h.htm";
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
	@Id(STIG)
	@Id(MASTER_DEFENDER)
	@Id(MEELE_DAMAGE_DEALER)
	@Id(DAGGER_MASTER)
	@Id(BOW_MASTER)
	@Id(BUFF_MASTER)
	@Id(MASTER_WIZARD)
	@Id(MASTER_SUMMONER)
	@Id(MASTER_HEALER)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		if (ask == 10556)
		{
			switch (reply)
			{
				case 1:
				{
					showHtmlFile(player, "stig_q10556_03.htm");
					break;
				}
				case 272:
				{
					qs.setCond(2);
					final Quest abelius = ScriptManager.getInstance().getScript(PowerOfAbelius.class.getSimpleName());
					if (abelius != null)
					{
						abelius.onEvent("enterInstance", npc, player);
					}
					break;
				}
				case 273:
				{
					qs.setCond(3);
					final Quest sapyros = ScriptManager.getInstance().getScript(PowerOfSapyros.class.getSimpleName());
					if (sapyros != null)
					{
						sapyros.onEvent("enterInstance", npc, player);
					}
					break;
				}
				case 274:
				{
					qs.setCond(4);
					final Quest kashnaga = ScriptManager.getInstance().getScript(PowerOfKashnaga.class.getSimpleName());
					if (kashnaga != null)
					{
						kashnaga.onEvent("enterInstance", npc, player);
					}
					break;
				}
				case 275:
				{
					qs.setCond(5);
					final Quest cranigg = ScriptManager.getInstance().getScript(PowerOfCranigg.class.getSimpleName());
					if (cranigg != null)
					{
						cranigg.onEvent("enterInstance", npc, player);
					}
					break;
				}
				case 276:
				{
					qs.setCond(6);
					final Quest naviarope = ScriptManager.getInstance().getScript(PowerOfNaviarope.class.getSimpleName());
					if (naviarope != null)
					{
						naviarope.onEvent("enterInstance", npc, player);
					}
					break;
				}
				case 277:
				{
					qs.setCond(7);
					final Quest raister = ScriptManager.getInstance().getScript(PowerOfRaister.class.getSimpleName());
					if (raister != null)
					{
						raister.onEvent("enterInstance", npc, player);
					}
					break;
				}
				case 278:
				{
					qs.setCond(8);
					final Quest soltkreig = ScriptManager.getInstance().getScript(PowerOfSoltkreig.class.getSimpleName());
					if (soltkreig != null)
					{
						soltkreig.onEvent("enterInstance", npc, player);
					}
					break;
				}
				case 279:
				{
					qs.setCond(9);
					final Quest lakcis = ScriptManager.getInstance().getScript(PowerOfLakcis.class.getSimpleName());
					if (lakcis != null)
					{
						lakcis.onEvent("enterInstance", npc, player);
					}
					break;
				}
			}
		}
		else if (ask == 10338)
		{
			switch (reply)
			{
				case 1:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 148);
							break;
						}
						case ELF:
						{
							changeBalthusClass(player, 150);
							break;
						}
						case DARK_ELF:
						{
							changeBalthusClass(player, 151);
							break;
						}
					}
					break;
				}
				case 10:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 149);
							break;
						}
					}
					break;
				}
				case 2:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 152);
							break;
						}
						case ORC:
						{
							changeBalthusClass(player, 154);
							break;
						}
						case DWARF:
						{
							changeBalthusClass(player, 156);
							break;
						}
						case KAMAEL:
						{
							changeBalthusClass(player, 157);
							break;
						}
					}
					break;
				}
				case 20:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 153);
							break;
						}
					}
					break;
				}
				case 21:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case ORC:
						{
							changeBalthusClass(player, 155);
							break;
						}
					}
					break;
				}
				case 3:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 158);
							break;
						}
						case ELF:
						{
							changeBalthusClass(player, 159);
							break;
						}
						case DARK_ELF:
						{
							changeBalthusClass(player, 160);
							break;
						}
						case DWARF:
						{
							changeBalthusClass(player, 161);
							break;
						}
					}
					break;
				}
				case 4:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 162);
							break;
						}
						case ELF:
						{
							changeBalthusClass(player, 163);
							break;
						}
						case DARK_ELF:
						{
							changeBalthusClass(player, 164);
							break;
						}
						case KAMAEL:
						{
							changeBalthusClass(player, 165);
							break;
						}
					}
					break;
				}
				case 5:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 166);
							break;
						}
						case ELF:
						{
							changeBalthusClass(player, 168);
							break;
						}
						case DARK_ELF:
						{
							changeBalthusClass(player, 169);
							break;
						}
						case KAMAEL:
						{
							changeBalthusClass(player, 170);
							break;
						}
					}
					break;
				}
				case 50:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 167);
							break;
						}
					}
					break;
				}
				case 6:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 171);
							break;
						}
						case ELF:
						{
							changeBalthusClass(player, 172);
							break;
						}
						case DARK_ELF:
						{
							changeBalthusClass(player, 173);
							break;
						}
						case ORC:
						{
							changeBalthusClass(player, 174);
							break;
						}
					}
					break;
				}
				case 60:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case ORC:
						{
							changeBalthusClass(player, 175);
							break;
						}
					}
					break;
				}
				case 7:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 176);
							break;
						}
						case ELF:
						{
							changeBalthusClass(player, 177);
							break;
						}
						case DARK_ELF:
						{
							changeBalthusClass(player, 178);
							break;
						}
					}
					break;
				}
				case 8:
				{
					qs.exitQuest(false, true);
					switch (player.getRace())
					{
						case HUMAN:
						{
							changeBalthusClass(player, 179);
							break;
						}
						case ELF:
						{
							changeBalthusClass(player, 180);
							break;
						}
						case DARK_ELF:
						{
							changeBalthusClass(player, 181);
							break;
						}
					}
					break;
				}
			}
		}
	}
	
	private void changeBalthusClass(Player player, int classId)
	{
		player.setPlayerClass(classId);
		player.setBaseClass(player.getActiveClass());
		player.getVariables().remove(PlayerVariables.PLAYER_RACE);
		showOnScreenMsg(player, NpcStringId.TALK_TO_THE_MONK_OF_CHAOS_NYOU_CAN_LEARN_ABOUT_THE_REVELATION_SKILLS, ExShowScreenMessage.TOP_CENTER, 5000, true);
		player.broadcastPacket(new MagicSkillUse(player, CommonSkill.PRODUCTION_CLAN_TRANSFER.getId(), CommonSkill.PRODUCTION_CLAN_TRANSFER.getLevel(), 1000, 0));
		final UserInfo ui = new UserInfo(player, false);
		ui.addComponentType(UserInfoType.BASIC_INFO);
		ui.addComponentType(UserInfoType.MAX_HPCPMP);
		player.sendPacket(ui);
		player.broadcastInfo();
		
		player.broadcastPacket(new SocialAction(player.getObjectId(), 20));
		giveItems(player, AGATHION_GRIFFIN, 1);
		for (Entry<PlayerClass, Integer> ent : AWAKE_POWER.entrySet())
		{
			if (player.getPlayerClass() == ent.getKey())
			{
				giveItems(player, ent.getValue(), 1);
				break;
			}
		}
		
		giveItems(player, CHAOS_POMANDER, 2);
		giveItems(player, GRADUATION_GIFT, 1);
		giveItems(player, PAULINA_EQUIPMENT_SET_R, 1);
		
		SkillTreeData.getInstance().cleanSkillUponAwakening(player);
		player.sendSkillList();
		
		// reset AP's
		for (SkillLearn sk : SkillTreeData.getInstance().getAbilitySkillTree().values())
		{
			final Skill skill = player.getKnownSkill(sk.getSkillId());
			if (skill != null)
			{
				player.removeSkill(skill);
			}
		}
		
		player.setAbilityPointsUsed(0);
		player.sendPacket(new ExAcquireAPSkillList(player));
		player.broadcastUserInfo();
		
		final Instance instance = player.getInstanceWorld();
		instance.finishInstance(1);
	}
}
