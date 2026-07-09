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
package handlers.bypasshandlers;

import java.util.Collection;
import java.util.Collections;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.handler.IBypassHandler;
import org.l2jmobius.gameserver.instancemanager.QuestManager;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.listeners.AbstractEventListener;
import org.l2jmobius.gameserver.model.quest.Quest;
import org.l2jmobius.gameserver.model.quest.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.NpcStringId.NSLocalisation;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ActionFailed;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

public class QuestLink implements IBypassHandler
{
	private static final String[] COMMANDS =
	{
		"Quest"
	};
	
	@Override
	public boolean useBypass(String command, Player player, Creature target)
	{
		String quest = "";
		try
		{
			quest = command.substring(5).trim();
		}
		catch (IndexOutOfBoundsException ioobe)
		{
		}
		if (quest.isEmpty())
		{
			showQuestWindow(player, (Npc) target);
		}
		else
		{
			final int questNameEnd = quest.indexOf(' ');
			if (questNameEnd == -1)
			{
				showQuestWindow(player, (Npc) target, quest);
			}
			else
			{
				player.processQuestEvent(quest.substring(0, questNameEnd), quest.substring(questNameEnd).trim());
			}
		}
		return true;
	}
	
	/**
	 * Open a choose quest window on client with all quests available of the Npc.<br>
	 * <br>
	 * <b><u>Actions</u>:</b><br>
	 * <li>Send a Server->Client NpcHtmlMessage containing the text of the Npc to the Player</li><br>
	 * @param player The Player that talk with the Npc
	 * @param npc The table containing quests of the Npc
	 * @param quests
	 */
	private void showQuestChooseWindow(Player player, Npc npc, Collection<Quest> quests)
	{
		final StringBuilder sbStarted = new StringBuilder(128);
		final StringBuilder sbCanStart = new StringBuilder(128);
		final StringBuilder sbCantStart = new StringBuilder(128);
		final StringBuilder sbCompleted = new StringBuilder(128);
		
		final Set<Quest> startingQuests = new HashSet<>();
		for (AbstractEventListener listener : npc.getListeners(EventType.ON_NPC_QUEST_START))
		{
			final Object owner = listener.getOwner();
			if (owner instanceof Quest)
			{
				startingQuests.add((Quest) owner);
			}
		}
		
		Collection<Quest> questList = quests;
		if (Config.ORDER_QUEST_LIST_BY_QUESTID)
		{
			final Map<Integer, Quest> orderedQuests = new TreeMap<>(); // Use TreeMap to order quests
			for (Quest q : questList)
			{
				orderedQuests.put(q.getId(), q);
			}
			questList = orderedQuests.values();
		}
		
		int startCount = 0;
		String startQuest = null;
		for (Quest quest : questList)
		{
			final QuestState qs = player.getQuestState(quest.getScriptName());
			if ((qs == null) || qs.isCreated() || (qs.isCompleted() && qs.isNowAvailable()))
			{
				final String startConditionHtml = quest.getStartConditionHtml(player, npc);
				if (((startConditionHtml != null) && startConditionHtml.isEmpty()) || !startingQuests.contains(quest))
				{
					continue;
				}
				else if (startingQuests.contains(quest) && quest.canStartQuest(player))
				{
					startCount++;
					startQuest = quest.getName();
					
					sbCanStart.append("<font color=\"bbaa88\">");
					sbCanStart.append("<button icon=\"quest\" align=\"left\" action=\"bypass npc_" + npc.getObjectId() + "_Quest " + quest.getName() + "\">");
					String localisation = quest.isCustomQuest() ? quest.getPath() : "<fstring>" + quest.getNpcStringId() + "01</fstring>";
					if (Config.MULTILANG_ENABLE)
					{
						final NpcStringId ns = NpcStringId.getNpcStringId(Integer.parseInt(quest.getNpcStringId() + "01"));
						if (ns != null)
						{
							final NSLocalisation nsl = ns.getLocalisation(player.getLang());
							if (nsl != null)
							{
								localisation = nsl.getLocalisation(Collections.emptyList());
							}
						}
					}
					sbCanStart.append(localisation);
					sbCanStart.append("</button></font>");
				}
				else
				{
					sbCantStart.append("<font color=\"a62f31\">");
					sbCantStart.append("<button icon=\"quest\" align=\"left\" action=\"bypass npc_" + npc.getObjectId() + "_Quest " + quest.getName() + "\">");
					String localisation = quest.isCustomQuest() ? quest.getPath() : "<fstring>" + quest.getNpcStringId() + "01</fstring>";
					if (Config.MULTILANG_ENABLE)
					{
						final NpcStringId ns = NpcStringId.getNpcStringId(Integer.parseInt(quest.getNpcStringId() + "01"));
						if (ns != null)
						{
							final NSLocalisation nsl = ns.getLocalisation(player.getLang());
							if (nsl != null)
							{
								localisation = nsl.getLocalisation(Collections.emptyList());
							}
						}
					}
					sbCantStart.append(localisation);
					sbCantStart.append("</button></font>");
				}
			}
			else if (Quest.getNoQuestMsg(player).equals(quest.onTalk(npc, player, true)))
			{
				continue;
			}
			else if (qs.isStarted())
			{
				startCount++;
				startQuest = quest.getName();

				sbStarted.append("<font color=\"ffdd66\">");
				sbStarted.append("<button icon=\"quest\" align=\"left\" action=\"bypass npc_" + npc.getObjectId() + "_Quest " + quest.getName() + "\">");
				String localisation = quest.isCustomQuest() ? quest.getPath() + " (In Progress)" : "<fstring>" + quest.getNpcStringId() + "02</fstring>";
				if (Config.MULTILANG_ENABLE)
				{
					final NpcStringId ns = NpcStringId.getNpcStringId(Integer.parseInt(quest.getNpcStringId() + "02"));
					if (ns != null)
					{
						final NSLocalisation nsl = ns.getLocalisation(player.getLang());
						if (nsl != null)
						{
							localisation = nsl.getLocalisation(Collections.emptyList());
						}
					}
				}
				sbStarted.append(localisation);
				sbStarted.append("</button></font>");
			}
			else if (qs.isCompleted())
			{
				sbCompleted.append("<font color=\"787878\">");
				sbCompleted.append("<button icon=\"quest\" align=\"left\" action=\"bypass npc_" + npc.getObjectId() + "_Quest " + quest.getName() + "\">");
				String localisation = quest.isCustomQuest() ? quest.getPath() + " (Done) " : "<fstring>" + quest.getNpcStringId() + "03</fstring>";
				if (Config.MULTILANG_ENABLE)
				{
					final NpcStringId ns = NpcStringId.getNpcStringId(Integer.parseInt(quest.getNpcStringId() + "03"));
					if (ns != null)
					{
						final NSLocalisation nsl = ns.getLocalisation(player.getLang());
						if (nsl != null)
						{
							localisation = nsl.getLocalisation(Collections.emptyList());
						}
					}
				}
				sbCompleted.append(localisation);
				sbCompleted.append("</button></font>");
			}
		}
		
		if (startCount == 1)
		{
			showQuestWindow(player, npc, startQuest);
			return;
		}
		
		String content;
		if ((sbStarted.length() > 0) || (sbCanStart.length() > 0) || (sbCantStart.length() > 0) || (sbCompleted.length() > 0))
		{
            String sb = "<html><body>" +
                    sbStarted +
                    sbCanStart +
                    sbCantStart +
                    sbCompleted +
                    "</body></html>";
			content = sb;
		}
		else
		{
			content = Quest.getNoQuestMsg(player);
		}
		
		// Send a Server->Client packet NpcHtmlMessage to the Player in order to display the message of the Npc
		content = content.replace("%objectId%", String.valueOf(npc.getObjectId()));
		player.sendPacket(new NpcHtmlMessage(npc.getObjectId(), content));
	}
	
	/**
	 * Open a quest window on client with the text of the Npc.<br>
	 * <br>
	 * <b><u>Actions</u>:</b><br>
	 * <ul>
	 * <li>Get the text of the quest state in the folder data/scripts/quests/questId/stateId.htm</li>
	 * <li>Send a Server->Client NpcHtmlMessage containing the text of the Npc to the Player</li>
	 * <li>Send a Server->Client ActionFailed to the Player in order to avoid that the client wait another packet</li>
	 * </ul>
	 * @param player the Player that talk with the {@code npc}
	 * @param npc the Npc that chats with the {@code player}
	 * @param questId the Id of the quest to display the message
	 */
	private void showQuestWindow(Player player, Npc npc, String questId)
	{
		String content = null;
		
		final Quest q = QuestManager.getInstance().getQuest(questId);
		
		// Get the state of the selected quest
		final QuestState qs = player.getQuestState(questId);
		if (q != null)
		{
			if (((q.getId() >= 1) && (q.getId() < 20000)) && ((player.getWeightPenalty() >= 3) || !player.isInventoryUnder90(true)))
			{
				player.sendPacket(SystemMessageId.UNABLE_TO_PROCESS_THIS_REQUEST_UNTIL_YOUR_INVENTORY_S_WEIGHT_AND_SLOT_COUNT_ARE_LESS_THAN_80_PERCENT_OF_CAPACITY);
				return;
			}
			
			if ((qs == null) && (q.getId() >= 1) && (q.getId() < 20000) && (player.getAllActiveQuests().size() > 40))
			{
				final NpcHtmlMessage html = new NpcHtmlMessage(npc.getObjectId());
				html.setFile(player, "data/html/fullquest.html");
				player.sendPacket(html);
				return;
			}
			
			q.notifyTalk(npc, player);
		}
		else
		{
			content = Quest.getNoQuestMsg(player); // no quests found
		}
		
		// Send a Server->Client packet NpcHtmlMessage to the Player in order to display the message of the Npc
		if (content != null)
		{
			content = content.replace("%objectId%", String.valueOf(npc.getObjectId()));
			player.sendPacket(new NpcHtmlMessage(npc.getObjectId(), content));
		}
		
		// Send a Server->Client ActionFailed to the Player in order to avoid that the client wait another packet
		player.sendPacket(ActionFailed.STATIC_PACKET);
	}
	
	/**
	 * Collect awaiting quests/start points and display a QuestChooseWindow (if several available) or QuestWindow.
	 * @param player the Player that talk with the {@code npc}.
	 * @param npc the Npc that chats with the {@code player}.
	 */
	private void showQuestWindow(Player player, Npc npc)
	{
		final Set<Quest> quests = new HashSet<>();
		for (AbstractEventListener listener : npc.getListeners(EventType.ON_NPC_TALK))
		{
			final Object owner = listener.getOwner();
			if (owner instanceof Quest quest)
			{
                if ((quest.getId() > 0) && (quest.getId() < 20000) && (quest.getId() != 255) && !Quest.getNoQuestMsg(player).equals(quest.onTalk(npc, player, true)))
				{
					quests.add(quest);
				}
			}
		}
		
		if (quests.size() > 1)
		{
			showQuestChooseWindow(player, npc, quests);
		}
		else if (quests.size() == 1)
		{
			showQuestWindow(player, npc, quests.iterator().next().getName());
		}
		else
		{
			showQuestWindow(player, npc, "");
		}
	}
	
	@Override
	public String[] getBypassList()
	{
		return COMMANDS;
	}
}
