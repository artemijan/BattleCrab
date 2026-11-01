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
package ai.others.TrainingCamp;

import java.util.concurrent.TimeUnit;

import org.l2jmobius.gameserver.config.TrainingCampConfig;
import org.l2jmobius.gameserver.data.holders.TrainingHolder;
import org.l2jmobius.gameserver.data.xml.ExperienceData;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogout;
import org.l2jmobius.gameserver.model.olympiad.OlympiadManager;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.zone.ZoneId;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;
import org.l2jmobius.gameserver.network.serverpackets.training.ExTrainingZone_Admission;
import org.l2jmobius.gameserver.network.serverpackets.training.ExTrainingZone_Leaving;

/**
 * TrainingCamp AI.
 * @author Gladicek, Mobius
 */
public class TrainingCamp extends Script
{
	// NPC
	private static final int RECRUITER = 4378;
	
	// Misc
	private static final Location TRAINING_LOCATION = new Location(-56516, 135938, -2672);
	
	private TrainingCamp()
	{
		addStartNpc(RECRUITER);
		addFirstTalkId(RECRUITER);
		addTalkId(RECRUITER);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		if (!TrainingCampConfig.TRAINING_CAMP_ENABLE || !checkConditions(player))
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "4378.htm":
			case "4378-02.htm":
			{
				htmltext = event;
				break;
			}
			case "info":
			{
				if (player.hasPremiumStatus() || !TrainingCampConfig.TRAINING_CAMP_PREMIUM_ONLY)
				{
					htmltext = "4378-02.htm";
				}
				else
				{
					htmltext = "4378-07.htm";
				}
				break;
			}
			case "enter":
			{
				final long trainingCampDuration = player.getTraingCampDuration();
				if (trainingCampDuration >= TrainingCampConfig.TRAINING_CAMP_MAX_DURATION)
				{
					player.sendPacket(SystemMessageId.YOU_HAVE_COMPLETED_THE_DAY_S_TRAINING);
				}
				else if (player.hasPremiumStatus() || !TrainingCampConfig.TRAINING_CAMP_PREMIUM_ONLY)
				{
					TrainingHolder holder = player.getTraingCampInfo();
					if ((holder != null) && (holder.getTrainingTime(TimeUnit.MINUTES) < 1))
					{
						holder = null;
					}
					
					if (holder == null)
					{
						player.disableAutoShotsAll();
						player.setLastLocation();
						player.disableAllSkills();
						player.setInvul(true);
						player.setInvisible(true);
						player.teleToLocation(TRAINING_LOCATION);
						player.setImmobilized(true);
						
						// @Sdw: Here we are supposed to send ExUserInfoEquipSlot with a fake equip of a SLS, feels ugly to me, not doing it.
						player.setTraingCampInfo(new TrainingHolder(player.getObjectId(), player.getClassIndex(), player.getLevel(), System.currentTimeMillis(), -1));
						final long timeRemaining = TrainingCampConfig.TRAINING_CAMP_MAX_DURATION - trainingCampDuration;
						player.sendPacket(new ExTrainingZone_Admission(player.getLevel(), 0, timeRemaining));
						startQuestTimer("finish", TimeUnit.SECONDS.toMillis(timeRemaining), npc, player);
					}
					else
					{
						htmltext = "4378-06.htm";
					}
					break;
				}
				else
				{
					htmltext = "4378-01.htm";
				}
				break;
			}
			case "4378-04.htm":
			{
				final TrainingHolder holder = player.getTraingCampInfo();
				if ((holder != null) && (holder.getObjectId() == player.getObjectId()))
				{
					if (holder.getClassIndex() == player.getClassIndex())
					{
						final long trainingTime = Math.max(0, holder.getTrainingTime(TimeUnit.MINUTES));
						if (trainingTime > 0)
						{
							final long expGained = (long) ((TrainingCampConfig.TRAINING_CAMP_EXP_MULTIPLIER * ((trainingTime * (ExperienceData.getInstance().getExpForLevel(holder.getLevel()) * ExperienceData.getInstance().getTrainingRate(holder.getLevel()))) / TrainingHolder.getTrainingDivider())) / 60);
							final long spGained = (long) (TrainingCampConfig.TRAINING_CAMP_SP_MULTIPLIER * (expGained / 250L));
							String html = getHtm(player, "4378-04.htm");
							html = html.replace("%training_level%", String.valueOf(holder.getLevel()));
							html = html.replace("%training_time%", String.valueOf(trainingTime));
							html = html.replace("%training_exp%", String.valueOf(expGained));
							html = html.replace("%training_sp%", String.valueOf(spGained));
							htmltext = html;
						}
						else
						{
							player.sendPacket(SystemMessageId.YOU_CANNOT_RECEIVE_REWARDS_FOR_TRAINING_IF_YOU_HAVE_TRAINED_FOR_LESS_THAN_1_MIN);
						}
					}
					else
					{
						player.sendPacket(SystemMessageId.YOU_CAN_ONLY_BE_REWARDED_AS_THE_CLASS_IN_WHICH_YOU_ENTERED_THE_TRAINING_CAMP);
					}
				}
				else
				{
					htmltext = "4378-05.htm";
				}
				break;
			}
			case "calculate":
			{
				final TrainingHolder holder = player.getTraingCampInfo();
				if ((holder != null) && (holder.getObjectId() == player.getObjectId()))
				{
					if (holder.getClassIndex() == player.getClassIndex())
					{
						final long trainingTime = holder.getTrainingTime(TimeUnit.MINUTES);
						if (trainingTime > 0)
						{
							player.sendPacket(SystemMessageId.CALCULATING_XP_AND_SP_OBTAINED_FROM_TRAINING);
							
							final long expGained = (long) ((TrainingCampConfig.TRAINING_CAMP_EXP_MULTIPLIER * ((trainingTime * (ExperienceData.getInstance().getExpForLevel(holder.getLevel()) * ExperienceData.getInstance().getTrainingRate(holder.getLevel()))) / TrainingHolder.getTrainingDivider())) / 60);
							final long spGained = (long) (TrainingCampConfig.TRAINING_CAMP_SP_MULTIPLIER * (expGained / 250L));
							player.addExpAndSp(expGained, spGained);
							
							final SystemMessage sysMsg = new SystemMessage(SystemMessageId.YOU_HAVE_COMPLETED_TRAINING_IN_THE_ROYAL_TRAINING_CAMP_AND_OBTAINED_S1_XP_AND_S2_SP);
							sysMsg.addLong(expGained);
							sysMsg.addLong(spGained);
							player.sendPacket(sysMsg);
						}
						else
						{
							player.sendPacket(SystemMessageId.YOU_CANNOT_RECEIVE_REWARDS_FOR_TRAINING_IF_YOU_HAVE_TRAINED_FOR_LESS_THAN_1_MIN);
						}
						
						player.setTraingCampDuration(player.getTraingCampDuration() + holder.getTrainingTime(TimeUnit.SECONDS));
						player.removeTraingCampInfo();
					}
					else
					{
						player.sendPacket(SystemMessageId.YOU_CAN_ONLY_BE_REWARDED_AS_THE_CLASS_IN_WHICH_YOU_ENTERED_THE_TRAINING_CAMP);
					}
				}
				break;
			}
			case "finish":
			{
				final TrainingHolder holder = player.getTraingCampInfo();
				if ((holder != null) && (holder.getObjectId() == player.getObjectId()))
				{
					holder.setEndTime(System.currentTimeMillis());
					player.setTraingCampInfo(holder);
					player.enableAllSkills();
					player.setInvul(false);
					player.setInvisible(false);
					player.setImmobilized(false);
					player.teleToLocation(player.getLastLocation());
					player.unsetLastLocation();
					player.sendPacket(ExTrainingZone_Leaving.STATIC_PACKET);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "4378.htm";
	}
	
	private boolean checkConditions(Player player)
	{
		if (player.getLevel() <= TrainingCampConfig.TRAINING_CAMP_MIN_LEVEL)
		{
			player.sendPacket(new SystemMessage(SystemMessageId.LV_S1_OR_ABOVE).addInt(TrainingCampConfig.TRAINING_CAMP_MIN_LEVEL));
			return false;
		}
		else if (player.getLevel() >= TrainingCampConfig.TRAINING_CAMP_MAX_LEVEL)
		{
			player.sendPacket(new SystemMessage(SystemMessageId.LV_S1_OR_BELOW).addInt(TrainingCampConfig.TRAINING_CAMP_MAX_LEVEL));
			return false;
		}
		else if (player.isFlyingMounted() || player.isTransformed())
		{
			player.sendPacket(SystemMessageId.YOU_CANNOT_ENTER_THE_TRAINING_CAMP_WITH_A_MOUNT_OR_IN_A_TRANSFORMED_STATE);
			return false;
		}
		
		final TrainingHolder holder = player.getTraingCampInfo();
		if ((holder != null) && (holder.getObjectId() != player.getObjectId()))
		{
			player.sendPacket(SystemMessageId.ONLY_ONE_CHARACTER_PER_ACCOUNT_MAY_ENTER_AT_ANY_TIME);
			return false;
		}
		else if (player.isInParty())
		{
			player.sendPacket(SystemMessageId.YOU_CANNOT_ENTER_THE_TRAINING_CAMP_WHILE_IN_A_PARTY_OR_USING_THE_AUTOMATIC_REPLACEMENT_SYSTEM);
			return false;
		}
		else if (player.isCursedWeaponEquipped() || (player.getReputation() < 0))
		{
			return false;
		}
		else if (player.isInDuel())
		{
			return false;
		}
		else if (player.isInOlympiadMode() || OlympiadManager.getInstance().isRegistered(player))
		{
			return false;
		}
		else if (player.isRegisteredOnEvent() || (player.getBlockCheckerArena() > -1)) // TODO underground coliseum and kratei checks.
		{
			return false;
		}
		else if (player.isInInstance())
		{
			return false;
		}
		else if (player.isInSiege())
		{
			return false;
		}
		else if (player.isInsideZone(ZoneId.SIEGE))
		{
			return false;
		}
		else if (player.isFishing())
		{
			return false;
		}
		else if (player.hasServitors())
		{
			return false;
		}
		
		return true;
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		final TrainingHolder holder = player.getTraingCampInfo();
		if (holder == null)
		{
			return;
		}
		
		if (holder.isValid(player) && holder.isTraining())
		{
			final long elapsedTime = holder.getElapsedTime();
			final long remainingPlayerTime = TrainingCampConfig.TRAINING_CAMP_MAX_DURATION - player.getTraingCampDuration();
			if (elapsedTime < remainingPlayerTime)
			{
				player.setLastLocation();
				player.disableAllSkills();
				player.setInvul(true);
				player.setInvisible(true);
				player.teleToLocation(TRAINING_LOCATION);
				player.setImmobilized(true);
				final long remainingDuration = remainingPlayerTime - elapsedTime;
				player.sendPacket(new ExTrainingZone_Admission(holder.getLevel(), TimeUnit.SECONDS.toMinutes(elapsedTime), remainingDuration));
				startQuestTimer("finish", TimeUnit.SECONDS.toMillis(remainingDuration), null, player);
			}
			else
			{
				holder.setEndTime(holder.getStartTime() + (remainingPlayerTime * 1000));
				player.setTraingCampInfo(holder);
				player.sendPacket(SystemMessageId.YOU_HAVE_COMPLETED_THE_DAY_S_TRAINING);
			}
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGOUT)
	@RegisterType(ListenerRegisterType.GLOBAL)
	public void onPlayerLogout(OnPlayerLogout event)
	{
		final Player player = event.getPlayer();
		final TrainingHolder holder = player.getTraingCampInfo();
		if (holder == null)
		{
			return;
		}
		
		if (holder.isValid(player) && holder.isTraining())
		{
			cancelQuestTimer("finish", null, player);
		}
	}
	
	public static void main(String[] args)
	{
		new TrainingCamp();
	}
}
