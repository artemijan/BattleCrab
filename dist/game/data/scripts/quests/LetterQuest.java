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
package quests;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.managers.CastleManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerBypass;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLevelChanged;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerPressTutorialMark;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.enums.HtmlActionScope;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;
import org.l2jmobius.gameserver.network.serverpackets.TutorialCloseHtml;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowHtml;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowQuestionMark;
import org.l2jmobius.gameserver.taskmanagers.AttackStanceTaskManager;

/**
 * Abstract class for quests "Letters from the Queen" and "Kekropus' Letter"
 * @author malyelfik, Trevor The Third
 */
public abstract class LetterQuest extends Quest
{
	private int _startSOE;
	private Location _startTeleport;
	private NpcStringId _startMessage;
	private String _startQuestSound;
	
	public LetterQuest(int questId)
	{
		super(questId);
		setStartLocation(_startSOE, _startTeleport);
	}
	
	/**
	 * Sets additional conditions that must be met to show question mark or start quest.
	 * @param player player trying start quest
	 * @return {@code true} when additional conditions are met, otherwise {@code false}
	 */
	public boolean canShowTutorialMark(Player player)
	{
		return true;
	}
	
	/**
	 * Sets start quest sound.<br>
	 * This sound will be played when player clicks on the tutorial question mark.
	 * @param sound name of sound
	 */
	public void setStartQuestSound(String sound)
	{
		_startQuestSound = sound;
	}
	
	/**
	 * Sets quest level restrictions.
	 * @param min minimum player's level to start quest
	 * @param max maximum player's level to start quest
	 */
	public void setLevel(int min, int max)
	{
		addCondLevel(min, max, "");
	}
	
	/**
	 * Sets start location of quest.<br>
	 * When player starts quest, he will receive teleport scroll with id {@code itemId}.<br>
	 * When tutorial window is displayed, he can also teleport to location {@code loc} using HTML bypass.
	 * @param itemId id of item which player gets on quest start
	 * @param loc place where player will be teleported
	 */
	public void setStartLocation(int itemId, Location loc)
	{
		_startSOE = itemId;
		_startTeleport = loc;
	}
	
	/**
	 * Sets if quest is only for Ertheia characters or not.
	 * @param value {@code true} means {@code Race.ERTHEIA}, {@code false} means other
	 */
	public void setIsErtheiaQuest(boolean value)
	{
		if (value)
		{
			addCondRace(Race.ERTHEIA, "");
			_startMessage = NpcStringId.QUEEN_NAVARI_HAS_SENT_A_LETTER_NCLICK_THE_QUESTION_MARK_ICON_TO_READ;
		}
		else
		{
			addCondNotRace(Race.ERTHEIA, "");
			_startMessage = NpcStringId.KEKROPUS_LETTER_HAS_ARRIVED_NCLICK_THE_QUESTION_MARK_ICON_TO_READ;
		}
	}
	
	public void setStartMessage(NpcStringId msg)
	{
		_startMessage = msg;
	}
	
	/**
	 * Gets teleport command associated with current quest.
	 * @return command in form Q<i>questId</i>_teleport (<i>questId</i> is replaced with original quest id)
	 */
	private String getTeleportCommand()
	{
		return "Q" + getId() + "_teleport";
	}
	
	@Override
	public boolean canStartQuest(Player player)
	{
		return canShowTutorialMark(player) && super.canStartQuest(player);
	}
	
	@RegisterEvent(EventType.ON_PLAYER_PRESS_TUTORIAL_MARK)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerPressTutorialMark(OnPlayerPressTutorialMark event)
	{
		final Player player = event.getPlayer();
		if ((event.getMarkId() == getId()) && canStartQuest(player))
		{
			final String html = getHtm(player, "popup.html").replace("%teleport%", getTeleportCommand());
			final QuestState qs = getQuestState(player, true);
			qs.startQuest();
			
			player.sendPacket(new PlaySound(3, _startQuestSound, 0, 0, 0, 0, 0));
			player.sendPacket(new TutorialShowHtml(html));
			giveItems(player, _startSOE, 1);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_BYPASS)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerBypass(OnPlayerBypass event)
	{
		final Player player = event.getPlayer();
		final QuestState qs = getQuestState(player, false);
		if (event.getCommand().equals(getTeleportCommand()))
		{
			if ((qs != null) && qs.isCond(1) && hasQuestItems(player, _startSOE))
			{
				if (CastleManager.getInstance().getCastles().stream().anyMatch(c -> c.getSiege().isInProgress()))
				{
					showOnScreenMsg(player, NpcStringId.YOU_MAY_NOT_TELEPORT_IN_MIDDLE_OF_A_SIEGE, ExShowScreenMessage.TOP_CENTER, 5000);
				}
				else if (player.isInParty())
				{
					showOnScreenMsg(player, NpcStringId.YOU_CANNOT_TELEPORT_IN_PARTY_STATUS, ExShowScreenMessage.TOP_CENTER, 5000);
				}
				else if (player.isInInstance())
				{
					showOnScreenMsg(player, NpcStringId.YOU_MAY_NOT_TELEPORT_WHILE_USING_INSTANCE_ZONE, ExShowScreenMessage.TOP_CENTER, 5000);
				}
				else if (AttackStanceTaskManager.getInstance().hasAttackStanceTask(player))
				{
					showOnScreenMsg(player, NpcStringId.YOU_CANNOT_TELEPORT_IN_COMBAT, ExShowScreenMessage.TOP_CENTER, 5000);
				}
				else if (player.isTransformed())
				{
					showOnScreenMsg(player, NpcStringId.YOU_CANNOT_TELEPORT_WHILE_IN_A_TRANSFORMED_STATE, ExShowScreenMessage.TOP_CENTER, 5000);
				}
				else if (player.isDead())
				{
					showOnScreenMsg(player, NpcStringId.YOU_CANNOT_TELEPORT_WHILE_YOU_ARE_DEAD, ExShowScreenMessage.TOP_CENTER, 5000);
				}
				else
				{
					player.teleToLocation(_startTeleport);
					takeItems(player, _startSOE, -1);
				}
			}
			
			player.sendPacket(TutorialCloseHtml.STATIC_PACKET);
			player.clearHtmlActions(HtmlActionScope.TUTORIAL_HTML);
		}
		
		if ((qs != null) && qs.isCond(1))
		{
			qs.setCond(2, true);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LEVEL_CHANGED)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLevelChanged(OnPlayerLevelChanged event)
	{
		if (PlayerConfig.DISABLE_TUTORIAL)
		{
			return;
		}
		
		final Player player = event.getPlayer();
		final QuestState qs = getQuestState(player, false);
		if ((qs == null) && (event.getOldLevel() < event.getNewLevel()) && canStartQuest(player) && !player.isInCategory(CategoryType.ALLOWED_BALTHUS_CLASSES))
		{
			player.sendPacket(new TutorialShowQuestionMark(getId(), 1));
			playSound(player, QuestSound.ITEMSOUND_QUEST_TUTORIAL);
			showOnScreenMsg(player, _startMessage, ExShowScreenMessage.TOP_CENTER, 10000);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLogin(OnPlayerLogin event)
	{
		if (PlayerConfig.DISABLE_TUTORIAL)
		{
			return;
		}
		
		final Player player = event.getPlayer();
		final QuestState qs = getQuestState(player, false);
		if ((qs == null) && canStartQuest(player) && !player.isInCategory(CategoryType.ALLOWED_BALTHUS_CLASSES))
		{
			player.sendPacket(new TutorialShowQuestionMark(getId(), 1));
			playSound(player, QuestSound.ITEMSOUND_QUEST_TUTORIAL);
			showOnScreenMsg(player, _startMessage, ExShowScreenMessage.TOP_CENTER, 10000);
		}
	}
	
	@Override
	public void onQuestAborted(Player player)
	{
		final QuestState qs = getQuestState(player, true);
		qs.startQuest();
		player.sendPacket(SystemMessageId.THIS_QUEST_CANNOT_BE_DELETED);
	}
}
