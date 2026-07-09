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
package handlers;

import java.lang.reflect.Method;
import java.util.HashMap;
import java.util.Map;
import java.util.Map.Entry;
import java.util.logging.Level;
import java.util.logging.Logger;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.handler.ActionHandler;
import org.l2jmobius.gameserver.handler.ActionShiftHandler;
import org.l2jmobius.gameserver.handler.AdminCommandHandler;
import org.l2jmobius.gameserver.handler.AffectObjectHandler;
import org.l2jmobius.gameserver.handler.AffectScopeHandler;
import org.l2jmobius.gameserver.handler.BypassHandler;
import org.l2jmobius.gameserver.handler.ChatHandler;
import org.l2jmobius.gameserver.handler.CommunityBoardHandler;
import org.l2jmobius.gameserver.handler.IHandler;
import org.l2jmobius.gameserver.handler.ItemHandler;
import org.l2jmobius.gameserver.handler.PlayerActionHandler;
import org.l2jmobius.gameserver.handler.PunishmentHandler;
import org.l2jmobius.gameserver.handler.TargetHandler;
import org.l2jmobius.gameserver.handler.UserCommandHandler;
import org.l2jmobius.gameserver.handler.VoicedCommandHandler;

import handlers.actionhandlers.ArtefactAction;
import handlers.actionhandlers.DecoyAction;
import handlers.actionhandlers.DoorAction;
import handlers.actionhandlers.ItemAction;
import handlers.actionhandlers.NpcAction;
import handlers.actionhandlers.PetAction;
import handlers.actionhandlers.PlayerAction;
import handlers.actionhandlers.StaticObjectAction;
import handlers.actionhandlers.SummonAction;
import handlers.actionhandlers.TrapAction;
import handlers.actionshifthandlers.DoorActionShift;
import handlers.actionshifthandlers.ItemActionShift;
import handlers.actionshifthandlers.NpcActionShift;
import handlers.actionshifthandlers.PlayerActionShift;
import handlers.actionshifthandlers.StaticObjectActionShift;
import handlers.actionshifthandlers.SummonActionShift;
import handlers.admincommandhandlers.AdminAdmin;
import handlers.admincommandhandlers.AdminAnnouncements;
import handlers.admincommandhandlers.AdminBBS;
import handlers.admincommandhandlers.AdminBuffs;
import handlers.admincommandhandlers.AdminCamera;
import handlers.admincommandhandlers.AdminCastle;
import handlers.admincommandhandlers.AdminChangeAccessLevel;
import handlers.admincommandhandlers.AdminClan;
import handlers.admincommandhandlers.AdminClanHall;
import handlers.admincommandhandlers.AdminCreateItem;
import handlers.admincommandhandlers.AdminCursedWeapons;
import handlers.admincommandhandlers.AdminDebug;
import handlers.admincommandhandlers.AdminDelete;
import handlers.admincommandhandlers.AdminDestroyItems;
import handlers.admincommandhandlers.AdminDisconnect;
import handlers.admincommandhandlers.AdminDoorControl;
import handlers.admincommandhandlers.AdminEditChar;
import handlers.admincommandhandlers.AdminEffects;
import handlers.admincommandhandlers.AdminElement;
import handlers.admincommandhandlers.AdminEnchant;
import handlers.admincommandhandlers.AdminEvents;
import handlers.admincommandhandlers.AdminExpSp;
import handlers.admincommandhandlers.AdminFakePlayers;
import handlers.admincommandhandlers.AdminFence;
import handlers.admincommandhandlers.AdminFightCalculator;
import handlers.admincommandhandlers.AdminFortSiege;
import handlers.admincommandhandlers.AdminGeodata;
import handlers.admincommandhandlers.AdminGm;
import handlers.admincommandhandlers.AdminGmChat;
import handlers.admincommandhandlers.AdminGmSpeed;
import handlers.admincommandhandlers.AdminGraciaSeeds;
import handlers.admincommandhandlers.AdminGrandBoss;
import handlers.admincommandhandlers.AdminHeal;
import handlers.admincommandhandlers.AdminHide;
import handlers.admincommandhandlers.AdminHtml;
import handlers.admincommandhandlers.AdminHwid;
import handlers.admincommandhandlers.AdminInstance;
import handlers.admincommandhandlers.AdminInstanceZone;
import handlers.admincommandhandlers.AdminInvul;
import handlers.admincommandhandlers.AdminKick;
import handlers.admincommandhandlers.AdminKill;
import handlers.admincommandhandlers.AdminLevel;
import handlers.admincommandhandlers.AdminLogin;
import handlers.admincommandhandlers.AdminManor;
import handlers.admincommandhandlers.AdminMenu;
import handlers.admincommandhandlers.AdminMessages;
import handlers.admincommandhandlers.AdminMissingHtmls;
import handlers.admincommandhandlers.AdminMobGroup;
import handlers.admincommandhandlers.AdminOlympiad;
import handlers.admincommandhandlers.AdminOnline;
import handlers.admincommandhandlers.AdminPForge;
import handlers.admincommandhandlers.AdminPathNode;
import handlers.admincommandhandlers.AdminPcCafePoints;
import handlers.admincommandhandlers.AdminPcCondOverride;
import handlers.admincommandhandlers.AdminPetition;
import handlers.admincommandhandlers.AdminPledge;
import handlers.admincommandhandlers.AdminPremium;
import handlers.admincommandhandlers.AdminPrimePoints;
import handlers.admincommandhandlers.AdminPunishment;
import handlers.admincommandhandlers.AdminQuest;
import handlers.admincommandhandlers.AdminReload;
import handlers.admincommandhandlers.AdminRepairChar;
import handlers.admincommandhandlers.AdminRes;
import handlers.admincommandhandlers.AdminRide;
import handlers.admincommandhandlers.AdminScan;
import handlers.admincommandhandlers.AdminServerInfo;
import handlers.admincommandhandlers.AdminShop;
import handlers.admincommandhandlers.AdminShowQuests;
import handlers.admincommandhandlers.AdminShutdown;
import handlers.admincommandhandlers.AdminSkill;
import handlers.admincommandhandlers.AdminSpawn;
import handlers.admincommandhandlers.AdminSummon;
import handlers.admincommandhandlers.AdminSuperHaste;
import handlers.admincommandhandlers.AdminTarget;
import handlers.admincommandhandlers.AdminTargetSay;
import handlers.admincommandhandlers.AdminTeleport;
import handlers.admincommandhandlers.AdminTest;
import handlers.admincommandhandlers.AdminTransform;
import handlers.admincommandhandlers.AdminVitality;
import handlers.admincommandhandlers.AdminZone;
import handlers.admincommandhandlers.AdminZones;
import handlers.bypasshandlers.Augment;
import handlers.bypasshandlers.Buy;
import handlers.bypasshandlers.ChangePlayerName;
import handlers.bypasshandlers.ChatLink;
import handlers.bypasshandlers.ClanWarehouse;
import handlers.bypasshandlers.FindPvP;
import handlers.bypasshandlers.Freight;
import handlers.bypasshandlers.ItemAuctionLink;
import handlers.bypasshandlers.Link;
import handlers.bypasshandlers.Loto;
import handlers.bypasshandlers.Multisell;
import handlers.bypasshandlers.NpcViewMod;
import handlers.bypasshandlers.Observation;
import handlers.bypasshandlers.PlayerHelp;
import handlers.bypasshandlers.PrivateWarehouse;
import handlers.bypasshandlers.QuestLink;
import handlers.bypasshandlers.ReleaseAttribute;
import handlers.bypasshandlers.SkillList;
import handlers.bypasshandlers.SupportBlessing;
import handlers.bypasshandlers.SupportMagic;
import handlers.bypasshandlers.TerritoryStatus;
import handlers.bypasshandlers.TutorialClose;
import handlers.bypasshandlers.VoiceCommand;
import handlers.bypasshandlers.Wear;
import handlers.chathandlers.ChatAlliance;
import handlers.chathandlers.ChatClan;
import handlers.chathandlers.ChatGeneral;
import handlers.chathandlers.ChatHeroVoice;
import handlers.chathandlers.ChatParty;
import handlers.chathandlers.ChatPartyMatchRoom;
import handlers.chathandlers.ChatPartyRoomAll;
import handlers.chathandlers.ChatPartyRoomCommander;
import handlers.chathandlers.ChatPetition;
import handlers.chathandlers.ChatShout;
import handlers.chathandlers.ChatTrade;
import handlers.chathandlers.ChatWhisper;
import handlers.chathandlers.ChatWorld;
import handlers.communityboard.ClanBoard;
import handlers.communityboard.DropSearchBoard;
import handlers.communityboard.FavoriteBoard;
import handlers.communityboard.FriendsBoard;
import handlers.communityboard.HomeBoard;
import handlers.communityboard.HomepageBoard;
import handlers.communityboard.MailBoard;
import handlers.communityboard.MemoBoard;
import handlers.communityboard.RegionBoard;
import handlers.itemhandlers.Appearance;
import handlers.itemhandlers.BeastSoulShot;
import handlers.itemhandlers.BeastSpiritShot;
import handlers.itemhandlers.BlessedSoulShots;
import handlers.itemhandlers.BlessedSpiritShot;
import handlers.itemhandlers.Book;
import handlers.itemhandlers.Bypass;
import handlers.itemhandlers.Calculator;
import handlers.itemhandlers.ChangeAttributeCrystal;
import handlers.itemhandlers.CharmOfCourage;
import handlers.itemhandlers.Elixir;
import handlers.itemhandlers.EnchantAttribute;
import handlers.itemhandlers.EnchantScrolls;
import handlers.itemhandlers.ExtractableItems;
import handlers.itemhandlers.FatedSupportBox;
import handlers.itemhandlers.FishShots;
import handlers.itemhandlers.Harvester;
import handlers.itemhandlers.ItemSkills;
import handlers.itemhandlers.ItemSkillsTemplate;
import handlers.itemhandlers.Maps;
import handlers.itemhandlers.MercTicket;
import handlers.itemhandlers.NicknameColor;
import handlers.itemhandlers.PetFood;
import handlers.itemhandlers.Recipes;
import handlers.itemhandlers.RollingDice;
import handlers.itemhandlers.Seed;
import handlers.itemhandlers.SoulShots;
import handlers.itemhandlers.SpecialXMas;
import handlers.itemhandlers.SpiritShot;
import handlers.itemhandlers.SummonItems;
import handlers.playeractions.AirshipAction;
import handlers.playeractions.BotReport;
import handlers.playeractions.InstanceZoneInfo;
import handlers.playeractions.PetAttack;
import handlers.playeractions.PetHold;
import handlers.playeractions.PetMove;
import handlers.playeractions.PetSkillUse;
import handlers.playeractions.PetStop;
import handlers.playeractions.PrivateStore;
import handlers.playeractions.Ride;
import handlers.playeractions.RunWalk;
import handlers.playeractions.ServitorAttack;
import handlers.playeractions.ServitorHold;
import handlers.playeractions.ServitorMode;
import handlers.playeractions.ServitorMove;
import handlers.playeractions.ServitorSkillUse;
import handlers.playeractions.ServitorStop;
import handlers.playeractions.SitStand;
import handlers.playeractions.SocialAction;
import handlers.playeractions.TacticalSignTarget;
import handlers.playeractions.TacticalSignUse;
import handlers.playeractions.TeleportBookmark;
import handlers.playeractions.UnsummonPet;
import handlers.playeractions.UnsummonServitor;
import handlers.punishmenthandlers.BanHandler;
import handlers.punishmenthandlers.ChatBanHandler;
import handlers.punishmenthandlers.JailHandler;
import handlers.targethandlers.AdvanceBase;
import handlers.targethandlers.Artillery;
import handlers.targethandlers.DoorTreasure;
import handlers.targethandlers.Enemy;
import handlers.targethandlers.EnemyNot;
import handlers.targethandlers.EnemyOnly;
import handlers.targethandlers.FortressFlagpole;
import handlers.targethandlers.Ground;
import handlers.targethandlers.HolyThing;
import handlers.targethandlers.Item;
import handlers.targethandlers.MyMentor;
import handlers.targethandlers.MyParty;
import handlers.targethandlers.None;
import handlers.targethandlers.NpcBody;
import handlers.targethandlers.Others;
import handlers.targethandlers.OwnerPet;
import handlers.targethandlers.PcBody;
import handlers.targethandlers.Self;
import handlers.targethandlers.Summon;
import handlers.targethandlers.Target;
import handlers.targethandlers.WyvernTarget;
import handlers.targethandlers.affectobject.All;
import handlers.targethandlers.affectobject.Clan;
import handlers.targethandlers.affectobject.Friend;
import handlers.targethandlers.affectobject.FriendPc;
import handlers.targethandlers.affectobject.HiddenPlace;
import handlers.targethandlers.affectobject.Invisible;
import handlers.targethandlers.affectobject.NotFriend;
import handlers.targethandlers.affectobject.NotFriendPc;
import handlers.targethandlers.affectobject.ObjectDeadNpcBody;
import handlers.targethandlers.affectobject.UndeadRealEnemy;
import handlers.targethandlers.affectobject.WyvernObject;
import handlers.targethandlers.affectscope.BalakasScope;
import handlers.targethandlers.affectscope.DeadParty;
import handlers.targethandlers.affectscope.DeadPartyPledge;
import handlers.targethandlers.affectscope.DeadPledge;
import handlers.targethandlers.affectscope.DeadUnion;
import handlers.targethandlers.affectscope.Fan;
import handlers.targethandlers.affectscope.FanPB;
import handlers.targethandlers.affectscope.Party;
import handlers.targethandlers.affectscope.PartyPledge;
import handlers.targethandlers.affectscope.Pledge;
import handlers.targethandlers.affectscope.PointBlank;
import handlers.targethandlers.affectscope.Range;
import handlers.targethandlers.affectscope.RangeSortByHp;
import handlers.targethandlers.affectscope.RingRange;
import handlers.targethandlers.affectscope.Single;
import handlers.targethandlers.affectscope.Square;
import handlers.targethandlers.affectscope.SquarePB;
import handlers.targethandlers.affectscope.StaticObjectScope;
import handlers.targethandlers.affectscope.SummonExceptMaster;
import handlers.usercommandhandlers.ChannelDelete;
import handlers.usercommandhandlers.ChannelInfo;
import handlers.usercommandhandlers.ChannelLeave;
import handlers.usercommandhandlers.ClanPenalty;
import handlers.usercommandhandlers.ClanWarsList;
import handlers.usercommandhandlers.Dismount;
import handlers.usercommandhandlers.ExperienceGain;
import handlers.usercommandhandlers.InstanceZone;
import handlers.usercommandhandlers.Loc;
import handlers.usercommandhandlers.Mount;
import handlers.usercommandhandlers.MyBirthday;
import handlers.usercommandhandlers.OlympiadStat;
import handlers.usercommandhandlers.PartyInfo;
import handlers.usercommandhandlers.SiegeStatus;
import handlers.usercommandhandlers.Time;
import handlers.usercommandhandlers.Unstuck;
import handlers.voicedcommandhandlers.AutoPlay;
import handlers.voicedcommandhandlers.AutoPotion;
import handlers.voicedcommandhandlers.Banking;
import handlers.voicedcommandhandlers.ChangePassword;
import handlers.voicedcommandhandlers.ChatAdmin;
import handlers.voicedcommandhandlers.Lang;
import handlers.voicedcommandhandlers.Offline;
import handlers.voicedcommandhandlers.OfflinePlay;
import handlers.voicedcommandhandlers.Online;
import handlers.voicedcommandhandlers.Premium;

/**
 * Master handler.
 * @author UnAfraid
 */
public class MasterHandler
{
	private static final Logger LOGGER = Logger.getLogger(MasterHandler.class.getName());
	
	private static final IHandler<?, ?>[] LOAD_INSTANCES =
	{
		ActionHandler.getInstance(),
		ActionShiftHandler.getInstance(),
		AdminCommandHandler.getInstance(),
		BypassHandler.getInstance(),
		ChatHandler.getInstance(),
		CommunityBoardHandler.getInstance(),
		ItemHandler.getInstance(),
		PunishmentHandler.getInstance(),
		UserCommandHandler.getInstance(),
		VoicedCommandHandler.getInstance(),
		TargetHandler.getInstance(),
		AffectObjectHandler.getInstance(),
		AffectScopeHandler.getInstance(),
		PlayerActionHandler.getInstance()
	};
	
	private static final Class<?>[][] HANDLERS =
	{
		{
			// Action Handlers
			ArtefactAction.class,
			DecoyAction.class,
			DoorAction.class,
			ItemAction.class,
			NpcAction.class,
			PlayerAction.class,
			PetAction.class,
			StaticObjectAction.class,
			SummonAction.class,
			TrapAction.class,
		},
		{
			// Action Shift Handlers
			DoorActionShift.class,
			ItemActionShift.class,
			NpcActionShift.class,
			PlayerActionShift.class,
			StaticObjectActionShift.class,
			SummonActionShift.class,
		},
		{
			// Admin Command Handlers
			AdminAdmin.class,
			AdminAnnouncements.class,
			AdminBBS.class,
			AdminBuffs.class,
			AdminCamera.class,
			AdminChangeAccessLevel.class,
			AdminClan.class,
			AdminClanHall.class,
			AdminCastle.class,
			AdminPcCondOverride.class,
			AdminCreateItem.class,
			AdminCursedWeapons.class,
			AdminDebug.class,
			AdminDelete.class,
			AdminDestroyItems.class,
			AdminDisconnect.class,
			AdminDoorControl.class,
			AdminEditChar.class,
			AdminEffects.class,
			AdminElement.class,
			AdminEnchant.class,
			AdminEvents.class,
			AdminExpSp.class,
			AdminFakePlayers.class,
			AdminFence.class,
			AdminFightCalculator.class,
			AdminFortSiege.class,
			AdminGeodata.class,
			AdminGm.class,
			AdminGmChat.class,
			AdminGmSpeed.class,
			AdminGraciaSeeds.class,
			AdminGrandBoss.class,
			AdminHeal.class,
			AdminHide.class,
			AdminHtml.class,
			AdminHwid.class,
			AdminInstance.class,
			AdminInstanceZone.class,
			AdminInvul.class,
			AdminKick.class,
			AdminKill.class,
			AdminLevel.class,
			AdminLogin.class,
			AdminManor.class,
			AdminMenu.class,
			AdminMessages.class,
			AdminMissingHtmls.class,
			AdminMobGroup.class,
			AdminOlympiad.class,
			AdminOnline.class,
			AdminPathNode.class,
			AdminPcCafePoints.class,
			AdminPetition.class,
			AdminPForge.class,
			AdminPledge.class,
			AdminZones.class,
			AdminPremium.class,
			AdminPrimePoints.class,
			AdminPunishment.class,
			AdminQuest.class,
			AdminReload.class,
			AdminRepairChar.class,
			AdminRes.class,
			AdminRide.class,
			AdminScan.class,
			AdminServerInfo.class,
			AdminShop.class,
			AdminShowQuests.class,
			AdminShutdown.class,
			AdminSkill.class,
			AdminSpawn.class,
			AdminSummon.class,
			AdminSuperHaste.class,
			AdminTarget.class,
			AdminTargetSay.class,
			AdminTeleport.class,
			AdminTest.class,
			AdminTransform.class,
			AdminVitality.class,
			AdminZone.class,
		},
		{
			// Bypass Handlers
			Augment.class,
			Buy.class,
			ChangePlayerName.class,
			ChatLink.class,
			ClanWarehouse.class,
			FindPvP.class,
			Freight.class,
			ItemAuctionLink.class,
			Link.class,
			Loto.class,
			Multisell.class,
			NpcViewMod.class,
			Observation.class,
			QuestLink.class,
			PlayerHelp.class,
			PrivateWarehouse.class,
			ReleaseAttribute.class,
			SkillList.class,
			SupportBlessing.class,
			SupportMagic.class,
			TerritoryStatus.class,
			TutorialClose.class,
			VoiceCommand.class,
			Wear.class,
		},
		{
			// Chat Handlers
			ChatGeneral.class,
			ChatAlliance.class,
			ChatClan.class,
			ChatHeroVoice.class,
			ChatParty.class,
			ChatPartyMatchRoom.class,
			ChatPartyRoomAll.class,
			ChatPartyRoomCommander.class,
			ChatPetition.class,
			ChatShout.class,
			ChatWhisper.class,
			ChatTrade.class,
			ChatWorld.class,
		},
		{
			// Community Board
			ClanBoard.class,
			FavoriteBoard.class,
			FriendsBoard.class,
			HomeBoard.class,
			HomepageBoard.class,
			MailBoard.class,
			MemoBoard.class,
			RegionBoard.class,
			DropSearchBoard.class,
		},
		{
			// Item Handlers
			Appearance.class,
			BeastSoulShot.class,
			BeastSpiritShot.class,
			BlessedSoulShots.class,
			BlessedSpiritShot.class,
			Book.class,
			Bypass.class,
			Calculator.class,
			ChangeAttributeCrystal.class,
			CharmOfCourage.class,
			Elixir.class,
			EnchantAttribute.class,
			EnchantScrolls.class,
			ExtractableItems.class,
			FatedSupportBox.class,
			FishShots.class,
			Harvester.class,
			ItemSkills.class,
			ItemSkillsTemplate.class,
			Maps.class,
			MercTicket.class,
			NicknameColor.class,
			PetFood.class,
			Recipes.class,
			RollingDice.class,
			Seed.class,
			SoulShots.class,
			SpecialXMas.class,
			SpiritShot.class,
			SummonItems.class,
		},
		{
			// Punishment Handlers
			BanHandler.class,
			ChatBanHandler.class,
			JailHandler.class,
		},
		{
			// User Command Handlers
			ClanPenalty.class,
			ClanWarsList.class,
			Dismount.class,
			ExperienceGain.class,
			Unstuck.class,
			InstanceZone.class,
			Loc.class,
			Mount.class,
			PartyInfo.class,
			Time.class,
			OlympiadStat.class,
			ChannelLeave.class,
			ChannelDelete.class,
			ChannelInfo.class,
			MyBirthday.class,
			SiegeStatus.class,
		},
		{
			// TODO: Add configuration options for this voiced commands:
			// CastleVCmd.class,
			// SetVCmd.class,
			Config.ENABLE_AUTO_PLAY ? AutoPlay.class : null,
			Config.BANKING_SYSTEM_ENABLED ? Banking.class : null,
			Config.CHAT_ADMIN ? ChatAdmin.class : null,
			Config.MULTILANG_ENABLE && Config.MULTILANG_VOICED_ALLOW ? Lang.class : null,
			Config.ALLOW_CHANGE_PASSWORD ? ChangePassword.class : null,
			Config.ENABLE_OFFLINE_PLAY_COMMAND ? OfflinePlay.class : null,
			Config.ENABLE_OFFLINE_COMMAND && (Config.OFFLINE_TRADE_ENABLE || Config.OFFLINE_CRAFT_ENABLE) ? Offline.class : null,
			Config.ENABLE_ONLINE_COMMAND ? Online.class : null,
			Config.PREMIUM_SYSTEM_ENABLED ? Premium.class : null,
			Config.AUTO_POTIONS_ENABLED ? AutoPotion.class : null,
		},
		{
			// Target Handlers
			AdvanceBase.class,
			Artillery.class,
			DoorTreasure.class,
			Enemy.class,
			EnemyNot.class,
			EnemyOnly.class,
			FortressFlagpole.class,
			Ground.class,
			HolyThing.class,
			Item.class,
			MyMentor.class,
			MyParty.class,
			None.class,
			NpcBody.class,
			Others.class,
			OwnerPet.class,
			PcBody.class,
			Self.class,
			Summon.class,
			Target.class,
			WyvernTarget.class,
		},
		{
			// Affect Objects
			All.class,
			Clan.class,
			Friend.class,
			FriendPc.class,
			HiddenPlace.class,
			Invisible.class,
			NotFriend.class,
			NotFriendPc.class,
			ObjectDeadNpcBody.class,
			UndeadRealEnemy.class,
			WyvernObject.class,
		},
		{
			// Affect Scopes
			BalakasScope.class,
			DeadParty.class,
			DeadPartyPledge.class,
			DeadPledge.class,
			DeadUnion.class,
			Fan.class,
			FanPB.class,
			Party.class,
			PartyPledge.class,
			Pledge.class,
			PointBlank.class,
			Range.class,
			RangeSortByHp.class,
			RingRange.class,
			Single.class,
			Square.class,
			SquarePB.class,
			StaticObjectScope.class,
			SummonExceptMaster.class,
		},
		{
			AirshipAction.class,
			BotReport.class,
			InstanceZoneInfo.class,
			PetAttack.class,
			PetHold.class,
			PetMove.class,
			PetSkillUse.class,
			PetStop.class,
			PrivateStore.class,
			Ride.class,
			RunWalk.class,
			ServitorAttack.class,
			ServitorHold.class,
			ServitorMode.class,
			ServitorMove.class,
			ServitorSkillUse.class,
			ServitorStop.class,
			SitStand.class,
			SocialAction.class,
			TacticalSignTarget.class,
			TacticalSignUse.class,
			TeleportBookmark.class,
			UnsummonPet.class,
			UnsummonServitor.class
		}
	};
	
	public static void main(String[] args)
	{
		LOGGER.info("Loading Handlers...");
		
		final Map<IHandler<?, ?>, Method> registerHandlerMethods = new HashMap<>();
		for (IHandler<?, ?> loadInstance : LOAD_INSTANCES)
		{
			registerHandlerMethods.put(loadInstance, null);
			for (Method method : loadInstance.getClass().getMethods())
			{
				if (method.getName().equals("registerHandler") && !method.isBridge())
				{
					registerHandlerMethods.put(loadInstance, method);
				}
			}
		}
		
		registerHandlerMethods.entrySet().stream().filter(e -> e.getValue() == null).forEach(e -> LOGGER.warning("Failed loading handlers of: " + e.getKey().getClass().getSimpleName() + " seems registerHandler function does not exist."));
		
		for (Class<?>[] classes : HANDLERS)
		{
			for (Class<?> c : classes)
			{
				if (c == null)
				{
					continue; // Disabled handler
				}
				
				try
				{
					final Object handler = c.getDeclaredConstructor().newInstance();
					for (Entry<IHandler<?, ?>, Method> entry : registerHandlerMethods.entrySet())
					{
						if ((entry.getValue() != null) && entry.getValue().getParameterTypes()[0].isInstance(handler))
						{
							entry.getValue().invoke(entry.getKey(), handler);
						}
					}
				}
				catch (Exception e)
				{
					LOGGER.log(Level.WARNING, "Failed loading handler: " + c.getSimpleName(), e);
				}
			}
		}
		
		for (IHandler<?, ?> loadInstance : LOAD_INSTANCES)
		{
			LOGGER.info(loadInstance.getClass().getSimpleName() + ": Loaded " + loadInstance.size() + " handlers.");
		}
		
		LOGGER.info("Handlers Loaded...");
	}
}
