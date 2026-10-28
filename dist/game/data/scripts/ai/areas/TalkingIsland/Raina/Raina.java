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
package ai.areas.TalkingIsland.Raina;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.List;
import java.util.Map;
import java.util.Map.Entry;
import java.util.Set;
import java.util.stream.Collectors;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.data.xml.CategoryData;
import org.l2jmobius.gameserver.data.xml.ClassListData;
import org.l2jmobius.gameserver.data.xml.SkillTreeData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.actor.enums.player.SubclassInfoType;
import org.l2jmobius.gameserver.model.actor.holders.player.SubClassHolder;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.AcquireSkillList;
import org.l2jmobius.gameserver.network.serverpackets.ExSubjobInfo;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;
import org.l2jmobius.gameserver.network.serverpackets.SocialAction;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

import quests.Q10385_RedThreadOfFate.Q10385_RedThreadOfFate;
import quests.Q10472_WindsOfFateEncroachingShadows.Q10472_WindsOfFateEncroachingShadows;

/**
 * Raina AI.
 * @author St3eT
 */
public class Raina extends Script
{
	// NPC
	private static final int RAINA = 33491;
	
	// Items
	private static final int SUBCLASS_CERTIFICATE = 30433;
	private static final int CHAOS_POMANDER = 37375;
	
	// Misc
	private static final Set<PlayerClass> mainSubclassSet;
	private static final Set<PlayerClass> neverSubclassed = EnumSet.of(PlayerClass.OVERLORD, PlayerClass.WARSMITH);
	private static final Set<PlayerClass> subclasseSet1 = EnumSet.of(PlayerClass.DARK_AVENGER, PlayerClass.PALADIN, PlayerClass.TEMPLE_KNIGHT, PlayerClass.SHILLIEN_KNIGHT);
	private static final Set<PlayerClass> subclasseSet2 = EnumSet.of(PlayerClass.TREASURE_HUNTER, PlayerClass.ABYSS_WALKER, PlayerClass.PLAINS_WALKER);
	private static final Set<PlayerClass> subclasseSet3 = EnumSet.of(PlayerClass.HAWKEYE, PlayerClass.SILVER_RANGER, PlayerClass.PHANTOM_RANGER);
	private static final Set<PlayerClass> subclasseSet4 = EnumSet.of(PlayerClass.WARLOCK, PlayerClass.ELEMENTAL_SUMMONER, PlayerClass.PHANTOM_SUMMONER);
	private static final Set<PlayerClass> subclasseSet5 = EnumSet.of(PlayerClass.SORCERER, PlayerClass.SPELLSINGER, PlayerClass.SPELLHOWLER);
	private static final EnumMap<PlayerClass, Set<PlayerClass>> subclassSetMap = new EnumMap<>(PlayerClass.class);
	static
	{
		final Set<PlayerClass> subclasses = CategoryData.getInstance().getCategoryByType(CategoryType.THIRD_CLASS_GROUP).stream().map(PlayerClass::getPlayerClass).collect(Collectors.toSet());
		subclasses.removeAll(neverSubclassed);
		mainSubclassSet = subclasses;
		subclassSetMap.put(PlayerClass.DARK_AVENGER, subclasseSet1);
		subclassSetMap.put(PlayerClass.PALADIN, subclasseSet1);
		subclassSetMap.put(PlayerClass.TEMPLE_KNIGHT, subclasseSet1);
		subclassSetMap.put(PlayerClass.SHILLIEN_KNIGHT, subclasseSet1);
		subclassSetMap.put(PlayerClass.TREASURE_HUNTER, subclasseSet2);
		subclassSetMap.put(PlayerClass.ABYSS_WALKER, subclasseSet2);
		subclassSetMap.put(PlayerClass.PLAINS_WALKER, subclasseSet2);
		subclassSetMap.put(PlayerClass.HAWKEYE, subclasseSet3);
		subclassSetMap.put(PlayerClass.SILVER_RANGER, subclasseSet3);
		subclassSetMap.put(PlayerClass.PHANTOM_RANGER, subclasseSet3);
		subclassSetMap.put(PlayerClass.WARLOCK, subclasseSet4);
		subclassSetMap.put(PlayerClass.ELEMENTAL_SUMMONER, subclasseSet4);
		subclassSetMap.put(PlayerClass.PHANTOM_SUMMONER, subclasseSet4);
		subclassSetMap.put(PlayerClass.SORCERER, subclasseSet5);
		subclassSetMap.put(PlayerClass.SPELLSINGER, subclasseSet5);
		subclassSetMap.put(PlayerClass.SPELLHOWLER, subclasseSet5);
	}
	
	private static final Map<CategoryType, Integer> classCloak = new EnumMap<>(CategoryType.class);
	static
	{
		classCloak.put(CategoryType.SIXTH_SIGEL_GROUP, 30310); // Abelius Cloak
		classCloak.put(CategoryType.SIXTH_TIR_GROUP, 30311); // Sapyros Cloak Grade
		classCloak.put(CategoryType.SIXTH_OTHEL_GROUP, 30312); // Ashagen Cloak Grade
		classCloak.put(CategoryType.SIXTH_YR_GROUP, 30313); // Cranigg Cloak Grade
		classCloak.put(CategoryType.SIXTH_FEOH_GROUP, 30314); // Soltkreig Cloak Grade
		classCloak.put(CategoryType.SIXTH_WYNN_GROUP, 30315); // Naviarope Cloak Grade
		classCloak.put(CategoryType.SIXTH_IS_GROUP, 30316); // Leister Cloak Grade
		classCloak.put(CategoryType.SIXTH_EOLH_GROUP, 30317); // Laksis Cloak Grade
	}
	
	private static final Map<CategoryType, Integer> powerItem = new EnumMap<>(CategoryType.class);
	static
	{
		powerItem.put(CategoryType.SIXTH_SIGEL_GROUP, 32264); // Abelius Power
		powerItem.put(CategoryType.SIXTH_TIR_GROUP, 32265); // Sapyros Power
		powerItem.put(CategoryType.SIXTH_OTHEL_GROUP, 32266); // Ashagen Power
		powerItem.put(CategoryType.SIXTH_YR_GROUP, 32267); // Cranigg Power
		powerItem.put(CategoryType.SIXTH_FEOH_GROUP, 32268); // Soltkreig Power
		powerItem.put(CategoryType.SIXTH_WYNN_GROUP, 32269); // Naviarope Power
		powerItem.put(CategoryType.SIXTH_IS_GROUP, 32270); // Leister Power
		powerItem.put(CategoryType.SIXTH_EOLH_GROUP, 32271); // Laksis Power
	}
	
	private static final List<PlayerClass> dualClassList = new ArrayList<>();
	static
	{
		dualClassList.addAll(Arrays.asList(PlayerClass.SIGEL_PHOENIX_KNIGHT, PlayerClass.SIGEL_HELL_KNIGHT, PlayerClass.SIGEL_EVA_TEMPLAR, PlayerClass.SIGEL_SHILLIEN_TEMPLAR));
		dualClassList.addAll(Arrays.asList(PlayerClass.TYRR_DUELIST, PlayerClass.TYRR_DREADNOUGHT, PlayerClass.TYRR_TITAN, PlayerClass.TYRR_GRAND_KHAVATARI, PlayerClass.TYRR_DOOMBRINGER));
		dualClassList.addAll(Arrays.asList(PlayerClass.OTHELL_ADVENTURER, PlayerClass.OTHELL_WIND_RIDER, PlayerClass.OTHELL_GHOST_HUNTER, PlayerClass.OTHELL_FORTUNE_SEEKER));
		dualClassList.addAll(Arrays.asList(PlayerClass.YUL_SAGITTARIUS, PlayerClass.YUL_MOONLIGHT_SENTINEL, PlayerClass.YUL_GHOST_SENTINEL, PlayerClass.YUL_TRICKSTER));
		dualClassList.addAll(Arrays.asList(PlayerClass.FEOH_ARCHMAGE, PlayerClass.FEOH_SOULTAKER, PlayerClass.FEOH_MYSTIC_MUSE, PlayerClass.FEOH_STORM_SCREAMER, PlayerClass.FEOH_SOUL_HOUND));
		dualClassList.addAll(Arrays.asList(PlayerClass.ISS_HIEROPHANT, PlayerClass.ISS_SWORD_MUSE, PlayerClass.ISS_SPECTRAL_DANCER, PlayerClass.ISS_DOOMCRYER));
		dualClassList.addAll(Arrays.asList(PlayerClass.WYNN_ARCANA_LORD, PlayerClass.WYNN_ELEMENTAL_MASTER, PlayerClass.WYNN_SPECTRAL_MASTER));
		dualClassList.addAll(Arrays.asList(PlayerClass.AEORE_CARDINAL, PlayerClass.AEORE_EVA_SAINT, PlayerClass.AEORE_SHILLIEN_SAINT));
	}
	
	// @formatter:off
	private static final int[] REAWAKEN_PRICE =
	{
		100_000_000, 90_000_000, 80_000_000, 70_000_000, 60_000_000, 50_000_000, 40_000_000, 30_000_000, 20_000_000, 10_000_000
	};
	// @formatter:on
	
	private Raina()
	{
		addStartNpc(RAINA);
		addFirstTalkId(RAINA);
		addTalkId(RAINA);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "33491-01.html":
			case "33491-02.html":
			case "33491-03.html":
			case "33491-04.html":
			case "33491-05.html":
			case "reawakenCancel.html":
			{
				htmltext = event;
				break;
			}
			case "addSubclass":
			{
				if (player.isTransformed())
				{
					htmltext = "noTransform.html";
				}
				else if (player.hasSummon())
				{
					htmltext = "noSummon.html";
				}
				else if (player.getRace() == Race.ERTHEIA)
				{
					htmltext = "noErtheia.html";
				}
				else if (!haveDoneQuest(player, false))
				{
					htmltext = "noQuest.html";
				}
				else if (!hasAllSubclassLeveled(player) || (player.getTotalSubClasses() >= PlayerConfig.MAX_SUBCLASS))
				{
					htmltext = "addFailed.html";
				}
				else if (!player.isInventoryUnder90(true) || (player.getWeightPenalty() >= 2))
				{
					htmltext = "inventoryLimit.html";
				}
				else
				{
					final Set<PlayerClass> availSubs = getAvailableSubClasses(player);
					final StringBuilder sb = new StringBuilder();
					final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "subclassList.html");
					if ((availSubs == null) || availSubs.isEmpty())
					{
						break;
					}
					
					for (PlayerClass subClass : availSubs)
					{
						if (subClass != null)
						{
							final int classId = subClass.getId();
							final int npcStringId = 11170000 + classId;
							sb.append("<fstring p1=\"0\" p2=\"" + classId + "\">" + npcStringId + "</fstring>");
						}
					}
					
					html.replace("%subclassList%", sb.toString());
					player.sendPacket(html);
				}
				break;
			}
			case "removeSubclass":
			{
				if (player.isTransformed())
				{
					htmltext = "noTransform.html";
				}
				else if (player.hasSummon())
				{
					htmltext = "noSummon.html";
				}
				else if (player.getRace() == Race.ERTHEIA)
				{
					htmltext = "noErtheia.html";
				}
				else if (!player.isInventoryUnder90(true) || (player.getWeightPenalty() >= 2))
				{
					htmltext = "inventoryLimit.html";
				}
				else if (player.getSubClasses().isEmpty())
				{
					htmltext = "noSubChange.html";
				}
				else
				{
					final StringBuilder sb = new StringBuilder();
					final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "subclassRemoveList.html");
					for (SubClassHolder subClass : player.getSubClasses().values())
					{
						if (subClass != null)
						{
							final int classId = subClass.getId();
							final int npcStringId = 11170000 + classId;
							sb.append("<fstring p1=\"2\" p2=\"" + subClass.getClassIndex() + "\">" + npcStringId + "</fstring>");
						}
					}
					
					html.replace("%removeList%", sb.toString());
					player.sendPacket(html);
				}
				break;
			}
			case "changeSubclass":
			{
				if (player.isTransformed())
				{
					htmltext = "noTransform.html";
				}
				else if (player.hasSummon())
				{
					htmltext = "noSummon.html";
				}
				else if (player.getRace() == Race.ERTHEIA)
				{
					htmltext = "noErtheia.html";
				}
				else if (!player.isInventoryUnder80(true) || (player.getWeightPenalty() >= 2))
				{
					htmltext = "inventoryLimit.html";
				}
				else if (player.getSubClasses().isEmpty())
				{
					htmltext = "noSubChange.html";
				}
				else if (!ownsAtLeastOneItem(player, SUBCLASS_CERTIFICATE))
				{
					htmltext = "noCertificate.html";
				}
				else
				{
					final StringBuilder sb = new StringBuilder();
					final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "subclassChangeList.html");
					for (SubClassHolder subClass : player.getSubClasses().values())
					{
						if (subClass != null)
						{
							final int classId = subClass.getId();
							final int npcStringId = 11170000 + classId;
							sb.append("<fstring p1=\"7\" p2=\"" + subClass.getClassIndex() + "\">" + npcStringId + "</fstring>");
						}
					}
					html.replace("%removeList%", sb.toString());
					player.sendPacket(html);
				}
				break;
			}
			case "ertheiaDualClass":
			{
				// TODO: Maybe html is different when you have 85level but you haven't completed quest
				if ((player.getRace() != Race.ERTHEIA) || (player.getLevel() < 85) || !player.isInCategory(CategoryType.SIXTH_CLASS_GROUP) || player.hasDualClass() || !haveDoneQuest(player, true))
				{
					htmltext = "addDualClassErtheiaFailed.html";
				}
				else
				{
					htmltext = "addDualClassErtheia.html";
				}
				break;
			}
			case "addDualClass_SIXTH_SIGEL_GROUP":
			case "addDualClass_SIXTH_TIR_GROUP":
			case "addDualClass_SIXTH_OTHEL_GROUP":
			case "addDualClass_SIXTH_YR_GROUP":
			case "addDualClass_SIXTH_FEOH_GROUP":
			case "addDualClass_SIXTH_IS_GROUP":
			case "addDualClass_SIXTH_WYNN_GROUP":
			case "addDualClass_SIXTH_EOLH_GROUP":
			{
				if ((player.getRace() != Race.ERTHEIA) || (player.getLevel() < 85) || !player.isInCategory(CategoryType.SIXTH_CLASS_GROUP) || player.hasDualClass() || !haveDoneQuest(player, true))
				{
					htmltext = "addDualClassErtheiaFailed.html";
					break;
				}
				
				final CategoryType cType = CategoryType.valueOf(event.replace("addDualClass_", ""));
				if (cType == null)
				{
					LOGGER.warning(getClass().getSimpleName() + ": Cannot parse CategoryType, event: " + event);
				}
				
				final StringBuilder sb = new StringBuilder();
				final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "addDualClassErtheiaList.html");
				for (PlayerClass dualClasses : getDualClasses(player, cType))
				{
					if (dualClasses != null)
					{
						sb.append("<button value=\"" + ClassListData.getInstance().getClass(dualClasses.getId()).getClassName() + "\" action=\"bypass -h menu_select?ask=6&reply=" + dualClasses.getId() + "\" width=\"200\" height=\"31\" back=\"L2UI_CT1.HtmlWnd_DF_Awake_Down\" fore=\"L2UI_CT1.HtmlWnd_DF_Awake\"><br>");
					}
				}
				
				html.replace("%dualclassList%", sb.toString());
				player.sendPacket(html);
				break;
			}
			case "reawakenDualclass":
			{
				if (player.isTransformed())
				{
					htmltext = "noTransform.html";
				}
				else if (player.hasSummon())
				{
					htmltext = "noSummon.html";
				}
				else if (!player.hasDualClass() || (player.getLevel() < 85) || !player.isDualClassActive() || !player.isAwakenedClass())
				{
					htmltext = "reawakenNoDual.html";
				}
				else if (!player.isInventoryUnder80(true) || (player.getWeightPenalty() >= 2))
				{
					htmltext = "inventoryLimit.html";
				}
				else
				{
					final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "reawaken.html");
					final int index = player.getLevel() > 94 ? REAWAKEN_PRICE.length - 1 : player.getLevel() - 85;
					html.replace("%price%", REAWAKEN_PRICE[index]);
					player.sendPacket(html);
				}
				break;
			}
			case "reawakenDualclassConfirm":
			{
				final int index = player.getLevel() > 94 ? REAWAKEN_PRICE.length - 1 : player.getLevel() - 85;
				if (player.isTransformed())
				{
					htmltext = "noTransform.html";
				}
				else if (player.hasSummon())
				{
					htmltext = "noSummon.html";
				}
				else if (!player.hasDualClass() || !player.isDualClassActive() || !player.isAwakenedClass())
				{
					htmltext = "reawakenNoDual.html";
				}
				else if ((player.getAdena() < REAWAKEN_PRICE[index]) || !ownsAtLeastOneItem(player, getCloakId(player)))
				{
					final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "reawakenNoFee.html");
					html.replace("%price%", REAWAKEN_PRICE[index]);
					player.sendPacket(html);
				}
				else
				{
					final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "reawakenList.html");
					player.sendPacket(html);
				}
				break;
			}
			case "reawaken_SIXTH_SIGEL_GROUP":
			case "reawaken_SIXTH_TIR_GROUP":
			case "reawaken_SIXTH_OTHEL_GROUP":
			case "reawaken_SIXTH_YR_GROUP":
			case "reawaken_SIXTH_FEOH_GROUP":
			case "reawaken_SIXTH_IS_GROUP":
			case "reawaken_SIXTH_WYNN_GROUP":
			case "reawaken_SIXTH_EOLH_GROUP":
			{
				final CategoryType cType = CategoryType.valueOf(event.replace("reawaken_", ""));
				if (cType == null)
				{
					LOGGER.warning(getClass().getSimpleName() + ": Cannot parse CategoryType, event: " + event);
				}
				
				final StringBuilder sb = new StringBuilder();
				final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "reawakenClassList.html");
				for (PlayerClass dualClasses : getDualClasses(player, cType))
				{
					if (dualClasses != null)
					{
						sb.append("<button value=\"" + ClassListData.getInstance().getClass(dualClasses.getId()).getClassName() + "\" action=\"bypass -h menu_select?ask=5&reply=" + dualClasses.getId() + "\" width=\"200\" height=\"31\" back=\"L2UI_CT1.HtmlWnd_DF_Awake_Down\" fore=\"L2UI_CT1.HtmlWnd_DF_Awake\"><br>");
					}
				}
				
				html.replace("%dualclassList%", sb.toString());
				player.sendPacket(html);
				break;
			}
			case "upgradeSubClassToDualClass":
			{
				if (PlayerConfig.ALT_GAME_DUALCLASS_WITHOUT_QUEST)
				{
					if (player.isTransformed())
					{
						htmltext = "noTransform.html";
					}
					else if (player.hasSummon())
					{
						htmltext = "noSummon.html";
					}
					else if (player.getRace() == Race.ERTHEIA)
					{
						htmltext = "noErtheia.html";
					}
					else if (!player.isInventoryUnder90(true) || (player.getWeightPenalty() >= 2))
					{
						htmltext = "inventoryLimit.html";
					}
					else if (player.hasDualClass() || !player.isSubClassActive() || (player.getLevel() < 80))
					{
						htmltext = "addDualClassWithoutQuestFailed.html";
					}
					else
					{
						player.getSubClasses().get(player.getClassIndex()).setDualClassActive(true);
						
						final SystemMessage msg = new SystemMessage(SystemMessageId.SUBCLASS_S1_HAS_BEEN_UPGRADED_TO_DUEL_CLASS_S2_CONGRATULATIONS);
						msg.addClassId(player.getPlayerClass().getId());
						msg.addClassId(player.getPlayerClass().getId());
						player.sendPacket(msg);
						
						player.sendPacket(new ExSubjobInfo(player, SubclassInfoType.CLASS_CHANGED));
						player.broadcastSocialAction(SocialAction.LEVEL_UP);
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(RAINA)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		
		switch (ask)
		{
			case 0: // Add subclass confirm menu
			{
				final int classId = event.getReply();
				final StringBuilder sb = new StringBuilder();
				final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "addConfirm.html");
				if (!isValidNewSubClass(player, classId))
				{
					return;
				}
				
				final int npcStringId = 11170000 + classId;
				sb.append("<fstring p1=\"1\" p2=\"" + classId + "\">" + npcStringId + "</fstring>");
				html.replace("%confirmButton%", sb.toString());
				player.sendPacket(html);
				break;
			}
			case 1: // Add subclass
			{
				final int classId = event.getReply();
				if (!isValidNewSubClass(player, classId))
				{
					return;
				}
				
				if (!player.addSubClass(classId, player.getTotalSubClasses() + 1, false))
				{
					return;
				}
				
				player.setActiveClass(player.getTotalSubClasses());
				player.broadcastUserInfo();
				player.sendPacket(new ExSubjobInfo(player, SubclassInfoType.NEW_SLOT_USED));
				player.sendPacket(SystemMessageId.THE_NEW_SUBCLASS_HAS_BEEN_ADDED);
				player.sendPacket(getNpcHtmlMessage(player, npc, "addSuccess.html"));
				break;
			}
			case 2: // Remove (change) subclass list
			{
				final int subclassIndex = event.getReply();
				final Set<PlayerClass> availSubs = getAvailableSubClasses(player);
				final StringBuilder sb = new StringBuilder();
				final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "removeSubclassList.html");
				if ((availSubs == null) || availSubs.isEmpty())
				{
					return;
				}
				
				for (PlayerClass subClass : availSubs)
				{
					if (subClass != null)
					{
						final int classId = subClass.getId();
						final int npcStringId = 11170000 + classId;
						sb.append("<fstring p1=\"3\" p2=\"" + classId + "\">" + npcStringId + "</fstring>");
					}
				}
				
				npc.getVariables().set("SUBCLASS_INDEX_" + player.getObjectId(), subclassIndex);
				html.replace("%subclassList%", sb.toString());
				player.sendPacket(html);
				break;
			}
			case 3: // Remove (change) subclass confirm menu
			{
				final int classId = event.getReply();
				final int classIndex = npc.getVariables().getInt("SUBCLASS_INDEX_" + player.getObjectId(), -1);
				if (classIndex < 0)
				{
					return;
				}
				
				final StringBuilder sb = new StringBuilder();
				final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "addConfirm2.html");
				final int npcStringId = 11170000 + classId;
				sb.append("<fstring p1=\"4\" p2=\"" + classId + "\">" + npcStringId + "</fstring>");
				html.replace("%confirmButton%", sb.toString());
				player.sendPacket(html);
				break;
			}
			case 4: // Remove (change) subclass
			{
				final int classId = event.getReply();
				final int classIndex = npc.getVariables().getInt("SUBCLASS_INDEX_" + player.getObjectId(), -1);
				if (classIndex < 0)
				{
					return;
				}
				
				if (player.modifySubClass(classIndex, classId, false))
				{
					player.abortCast();
					player.stopAllEffectsExceptThoseThatLastThroughDeath();
					player.stopAllEffects();
					player.stopCubics();
					player.setActiveClass(classIndex);
					player.sendPacket(new ExSubjobInfo(player, SubclassInfoType.CLASS_CHANGED));
					player.sendPacket(getNpcHtmlMessage(player, npc, "addSuccess.html"));
					player.sendPacket(SystemMessageId.THE_NEW_SUBCLASS_HAS_BEEN_ADDED);
				}
				break;
			}
			case 5: // Reawaken (change dual class)
			{
				final int classId = event.getReply();
				if (player.isTransformed() || player.hasSummon() || (!player.hasDualClass() || !player.isDualClassActive() || !player.isAwakenedClass()))
				{
					break;
				}
				
				// Validating classId
				if (!getDualClasses(player, null).contains(PlayerClass.getPlayerClass(classId)))
				{
					break;
				}
				
				final int index = player.getLevel() > 94 ? REAWAKEN_PRICE.length - 1 : player.getLevel() - 85;
				if ((player.getAdena() < REAWAKEN_PRICE[index]) || !ownsAtLeastOneItem(player, getCloakId(player)))
				{
					final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "reawakenNoFee.html");
					html.replace("%price%", REAWAKEN_PRICE[index]);
					player.sendPacket(html);
					break;
				}
				
				player.reduceAdena(ItemProcessType.FEE, REAWAKEN_PRICE[index], npc, true);
				takeItems(player, getCloakId(player), 1);
				
				final int classIndex = player.getClassIndex();
				if (player.modifySubClass(classIndex, classId, true))
				{
					player.abortCast();
					player.stopAllEffectsExceptThoseThatLastThroughDeath();
					player.stopAllEffects();
					player.stopCubics();
					player.setActiveClass(classIndex);
					player.sendPacket(new ExSubjobInfo(player, SubclassInfoType.CLASS_CHANGED));
					player.sendPacket(getNpcHtmlMessage(player, npc, "reawakenSuccess.html"));
					SkillTreeData.getInstance().cleanSkillUponAwakening(player);
					player.restoreDualSkills();
					player.sendPacket(new AcquireSkillList(player));
					player.sendSkillList();
					giveItems(player, getCloakId(player), 1);
					takeItems(player, CHAOS_POMANDER, -1);
					giveItems(player, CHAOS_POMANDER, 2);
				}
				break;
			}
			case 6: // Add dual class for ertheia
			{
				final int classId = event.getReply();
				if (player.isTransformed() || player.hasSummon() || player.hasDualClass())
				{
					break;
				}
				
				final QuestState qs = player.getQuestState(Q10472_WindsOfFateEncroachingShadows.class.getSimpleName());
				if (((qs == null) || !qs.isCompleted()) && !PlayerConfig.ALT_GAME_SUBCLASS_WITHOUT_QUESTS)
				{
					break;
				}
				
				// Validating classId
				if (!getDualClasses(player, null).contains(PlayerClass.getPlayerClass(classId)))
				{
					break;
				}
				
				if (player.addSubClass(classId, player.getTotalSubClasses() + 1, true))
				{
					player.setActiveClass(player.getTotalSubClasses());
					player.sendPacket(new ExSubjobInfo(player, SubclassInfoType.NEW_SLOT_USED));
					player.sendPacket(SystemMessageId.THE_NEW_SUBCLASS_HAS_BEEN_ADDED);
					player.sendPacket(getNpcHtmlMessage(player, npc, "addSuccess.html"));
					SkillTreeData.getInstance().cleanSkillUponAwakening(player);
					player.restoreDualSkills();
					player.sendPacket(new AcquireSkillList(player));
					player.sendSkillList();
					giveItems(player, getPowerItemId(player), 1);
					takeItems(player, CHAOS_POMANDER, -1);
					giveItems(player, CHAOS_POMANDER, 2);
				}
				break;
			}
			case 7: // change subclass list
			{
				final int subclassIndex = event.getReply();
				final Set<PlayerClass> availSubs = getAvailableSubClasses(player);
				final StringBuilder sb = new StringBuilder();
				final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "changeSubclassList.html");
				if ((availSubs == null) || availSubs.isEmpty())
				{
					return;
				}
				
				for (PlayerClass subClass : availSubs)
				{
					if (subClass != null)
					{
						final int classId = subClass.getId();
						final int npcStringId = 11170000 + classId;
						sb.append("<fstring p1=\"8\" p2=\"" + classId + "\">" + npcStringId + "</fstring>");
					}
				}
				npc.getVariables().set("SUBCLASS_INDEX_" + player.getObjectId(), subclassIndex);
				html.replace("%subclassList%", sb.toString());
				player.sendPacket(html);
				break;
			}
			case 8: // change subclass confirm menu
			{
				final int classId = event.getReply();
				final int classIndex = npc.getVariables().getInt("SUBCLASS_INDEX_" + player.getObjectId(), -1);
				if (classIndex < 0)
				{
					return;
				}
				
				final StringBuilder sb = new StringBuilder();
				final NpcHtmlMessage html = getNpcHtmlMessage(player, npc, "addConfirm2.html");
				final int npcStringId = 11170000 + classId;
				sb.append("<fstring p1=\"9\" p2=\"" + classId + "\">" + npcStringId + "</fstring>");
				html.replace("%confirmButton%", sb.toString());
				player.sendPacket(html);
				break;
			}
			case 9: // change subclass
			{
				final int classId = event.getReply();
				final int classIndex = npc.getVariables().getInt("SUBCLASS_INDEX_" + player.getObjectId(), -1);
				if (classIndex < 0)
				{
					return;
				}
				
				final SubClassHolder subClass = player.getSubClasses().get(classIndex);
				if (subClass != null)
				{
					final long subChangeLevel = subClass.getLevel();
					final long subChangeExp = subClass.getExp();
					if (player.modifySubClass(classIndex, classId, false))
					{
						player.abortCast();
						player.stopAllEffectsExceptThoseThatLastThroughDeath();
						player.stopAllEffects();
						player.stopCubics();
						player.setActiveClass(classIndex);
						
						final SubClassHolder newClass = player.getSubClasses().get(classIndex);
						newClass.setExp(subChangeExp);
						newClass.setLevel((byte) subChangeLevel);
						
						player.sendPacket(new ExSubjobInfo(player, SubclassInfoType.CLASS_CHANGED));
						player.sendPacket(getNpcHtmlMessage(player, npc, "addSuccess.html"));
						player.sendPacket(SystemMessageId.THE_NEW_SUBCLASS_HAS_BEEN_ADDED);
					}
				}
				break;
			}
		}
	}
	
	/**
	 * Returns list of available subclasses Base class and already used subclasses removed
	 * @param player
	 * @return
	 */
	private Set<PlayerClass> getAvailableSubClasses(Player player)
	{
		final int currentBaseId = player.getBaseClass();
		final PlayerClass baseCID = PlayerClass.getPlayerClass(currentBaseId);
		final int baseClassId = (CategoryData.getInstance().isInCategory(CategoryType.FOURTH_CLASS_GROUP, baseCID.getId()) || CategoryData.getInstance().isInCategory(CategoryType.FIFTH_CLASS_GROUP, baseCID.getId()) || CategoryData.getInstance().isInCategory(CategoryType.SIXTH_CLASS_GROUP, baseCID.getId())) ? baseCID.getParent().getId() : currentBaseId;
		final Set<PlayerClass> availSubs = getSubclasses(player, baseClassId);
		if ((availSubs != null) && !availSubs.isEmpty())
		{
			for (PlayerClass pclass : availSubs)
			{
				// scan for already used subclasses
				final int availClassId = pclass.getId();
				final PlayerClass cid = PlayerClass.getPlayerClass(availClassId);
				for (SubClassHolder subList : player.getSubClasses().values())
				{
					final PlayerClass subId = PlayerClass.getPlayerClass(subList.getId());
					if (subId.equalsOrChildOf(cid))
					{
						availSubs.remove(cid);
						break;
					}
				}
			}
		}
		
		return availSubs;
	}
	
	private boolean haveDoneQuest(Player player, boolean isErtheia)
	{
		final QuestState qs = isErtheia ? player.getQuestState(Q10472_WindsOfFateEncroachingShadows.class.getSimpleName()) : player.getQuestState(Q10385_RedThreadOfFate.class.getSimpleName());
		return (((qs != null) && qs.isCompleted()) || PlayerConfig.ALT_GAME_SUBCLASS_WITHOUT_QUESTS);
	}
	
	/**
	 * Check new subclass classId for validity. Base class not added into allowed subclasses.
	 * @param player
	 * @param classId
	 * @return
	 */
	private boolean isValidNewSubClass(Player player, int classId)
	{
		final PlayerClass cid = PlayerClass.getPlayerClass(classId);
		PlayerClass subClassId;
		for (SubClassHolder subList : player.getSubClasses().values())
		{
			subClassId = PlayerClass.getPlayerClass(subList.getId());
			if (subClassId.equalsOrChildOf(cid))
			{
				return false;
			}
		}
		
		// get player base class
		final int currentBaseId = player.getBaseClass();
		final PlayerClass baseCID = PlayerClass.getPlayerClass(currentBaseId);
		
		// we need 2nd occupation ID
		final int baseClassId = (CategoryData.getInstance().isInCategory(CategoryType.FOURTH_CLASS_GROUP, baseCID.getId()) || CategoryData.getInstance().isInCategory(CategoryType.FIFTH_CLASS_GROUP, baseCID.getId()) || CategoryData.getInstance().isInCategory(CategoryType.SIXTH_CLASS_GROUP, baseCID.getId())) ? baseCID.getParent().getId() : currentBaseId;
		final Set<PlayerClass> availSubs = getSubclasses(player, baseClassId);
		if ((availSubs == null) || availSubs.isEmpty())
		{
			return false;
		}
		
		boolean found = false;
		for (PlayerClass pclass : availSubs)
		{
			if (pclass.getId() == classId)
			{
				found = true;
				break;
			}
		}
		
		return found;
	}
	
	private boolean hasAllSubclassLeveled(Player player)
	{
		boolean leveled = true;
		for (SubClassHolder sub : player.getSubClasses().values())
		{
			if ((sub != null) && (sub.getLevel() < 75))
			{
				leveled = false;
			}
		}
		
		return leveled;
	}
	
	public List<PlayerClass> getAvailableDualclasses(Player player)
	{
		final List<PlayerClass> dualClasses = new ArrayList<>();
		for (PlayerClass ClassId : PlayerClass.values())
		{
			if ((ClassId.getRace() != Race.ERTHEIA) && CategoryData.getInstance().isInCategory(CategoryType.SIXTH_CLASS_GROUP, ClassId.getId()) && (ClassId.getId() != player.getPlayerClass().getId()))
			{
				dualClasses.add(ClassId);
			}
		}
		
		return dualClasses;
	}
	
	private List<PlayerClass> getDualClasses(Player player, CategoryType cType)
	{
		final List<PlayerClass> tempList = new ArrayList<>();
		final int baseClassId = player.getBaseClass();
		final int dualClassId = player.getPlayerClass().getId();
		for (PlayerClass temp : dualClassList)
		{
			if ((temp.getId() != baseClassId) && (temp.getId() != dualClassId) && ((cType == null) || CategoryData.getInstance().isInCategory(cType, temp.getId())))
			{
				tempList.add(temp);
			}
		}
		
		return tempList;
	}
	
	public final Set<PlayerClass> getSubclasses(Player player, int classId)
	{
		Set<PlayerClass> subclasses = null;
		final PlayerClass pClass = PlayerClass.getPlayerClass(classId);
		if (CategoryData.getInstance().isInCategory(CategoryType.THIRD_CLASS_GROUP, classId) || (CategoryData.getInstance().isInCategory(CategoryType.FOURTH_CLASS_GROUP, classId)))
		{
			subclasses = EnumSet.copyOf(mainSubclassSet);
			subclasses.remove(pClass);
			
			// Ertheia classes cannot be subclassed and only Kamael can take Kamael classes as subclasses.
			for (PlayerClass cid : PlayerClass.values())
			{
				if ((cid.getRace() == Race.ERTHEIA) || ((cid.getRace() == Race.KAMAEL) && (player.getRace() != Race.KAMAEL)))
				{
					subclasses.remove(cid);
				}
			}
			
			if (player.getRace() == Race.KAMAEL)
			{
				if (player.getAppearance().isFemale())
				{
					subclasses.remove(PlayerClass.MALE_SOULBREAKER);
				}
				else
				{
					subclasses.remove(PlayerClass.FEMALE_SOULBREAKER);
				}
				
				if (!player.getSubClasses().containsKey(2) || (player.getSubClasses().get(2).getLevel() < 75))
				{
					subclasses.remove(PlayerClass.INSPECTOR);
				}
			}
			
			final Set<PlayerClass> unavailableClasses = subclassSetMap.get(pClass);
			if (unavailableClasses != null)
			{
				subclasses.removeAll(unavailableClasses);
			}
		}
		
		if (subclasses != null)
		{
			final PlayerClass currClassId = player.getPlayerClass();
			for (PlayerClass tempClass : subclasses)
			{
				if (currClassId.equalsOrChildOf(tempClass))
				{
					subclasses.remove(tempClass);
				}
			}
		}
		
		return subclasses;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		if (PlayerConfig.ALT_GAME_DUALCLASS_WITHOUT_QUEST)
		{
			return "addDualClassWithoutQuest.html";
		}
		
		return "33491.html";
	}
	
	private NpcHtmlMessage getNpcHtmlMessage(Player player, Npc npc, String fileName)
	{
		final NpcHtmlMessage html = new NpcHtmlMessage(npc.getObjectId());
		final String text = getHtm(player, fileName);
		if (text == null)
		{
			LOGGER.info("Cannot find HTML file for " + Raina.class.getSimpleName() + " AI: " + fileName);
			return null;
		}
		
		html.setHtml(text);
		return html;
	}
	
	private int getCloakId(Player player)
	{
		return classCloak.entrySet().stream().filter(e -> player.isInCategory(e.getKey())).mapToInt(Entry::getValue).findFirst().orElse(0);
	}
	
	private int getPowerItemId(Player player)
	{
		return powerItem.entrySet().stream().filter(e -> player.isInCategory(e.getKey())).mapToInt(Entry::getValue).findFirst().orElse(0);
	}
	
	public static void main(String[] args)
	{
		new Raina();
	}
}