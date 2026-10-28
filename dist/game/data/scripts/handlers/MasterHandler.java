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
package handlers;

import java.lang.reflect.Method;
import java.util.HashMap;
import java.util.Map;
import java.util.Map.Entry;
import java.util.logging.Level;
import java.util.logging.Logger;

import org.l2jmobius.gameserver.config.custom.AutoPlayConfig;
import org.l2jmobius.gameserver.config.custom.AutoPotionsConfig;
import org.l2jmobius.gameserver.config.custom.BankingConfig;
import org.l2jmobius.gameserver.config.custom.ChatModerationConfig;
import org.l2jmobius.gameserver.config.custom.MultilingualSupportConfig;
import org.l2jmobius.gameserver.config.custom.OfflinePlayConfig;
import org.l2jmobius.gameserver.config.custom.OfflineTradeConfig;
import org.l2jmobius.gameserver.config.custom.OnlineInfoConfig;
import org.l2jmobius.gameserver.config.custom.PasswordChangeConfig;
import org.l2jmobius.gameserver.config.custom.PremiumSystemConfig;
import org.l2jmobius.gameserver.config.custom.WeddingConfig;
import org.l2jmobius.gameserver.handler.ActionClickHandler;
import org.l2jmobius.gameserver.handler.ActionShiftHandler;
import org.l2jmobius.gameserver.handler.ActionUserHandler;
import org.l2jmobius.gameserver.handler.AdminCommandHandler;
import org.l2jmobius.gameserver.handler.AffectObjectHandler;
import org.l2jmobius.gameserver.handler.AffectScopeHandler;
import org.l2jmobius.gameserver.handler.BypassHandler;
import org.l2jmobius.gameserver.handler.ChatHandler;
import org.l2jmobius.gameserver.handler.CommunityBoardHandler;
import org.l2jmobius.gameserver.handler.IHandler;
import org.l2jmobius.gameserver.handler.ItemHandler;
import org.l2jmobius.gameserver.handler.PunishmentHandler;
import org.l2jmobius.gameserver.handler.TargetHandler;
import org.l2jmobius.gameserver.handler.UserCommandHandler;
import org.l2jmobius.gameserver.handler.VoicedCommandHandler;

import handlers.actions.click.ArtefactClick;
import handlers.actions.click.DecoyClick;
import handlers.actions.click.DoorClick;
import handlers.actions.click.ItemClick;
import handlers.actions.click.NpcClick;
import handlers.actions.click.PetClick;
import handlers.actions.click.PlayerClick;
import handlers.actions.click.StaticObjectClick;
import handlers.actions.click.SummonClick;
import handlers.actions.click.TrapClick;
import handlers.actions.shiftclick.DoorShiftClick;
import handlers.actions.shiftclick.ItemShiftClick;
import handlers.actions.shiftclick.NpcShiftClick;
import handlers.actions.shiftclick.PlayerShiftClick;
import handlers.actions.shiftclick.StaticObjectShiftClick;
import handlers.actions.shiftclick.SummonShiftClick;
import handlers.actions.user.AirshipAction;
import handlers.actions.user.BotReport;
import handlers.actions.user.InstanceZoneInfo;
import handlers.actions.user.PetAttack;
import handlers.actions.user.PetHold;
import handlers.actions.user.PetMove;
import handlers.actions.user.PetSkillUse;
import handlers.actions.user.PetStop;
import handlers.actions.user.PrivateStore;
import handlers.actions.user.Ride;
import handlers.actions.user.RunWalk;
import handlers.actions.user.ServitorAttack;
import handlers.actions.user.ServitorHold;
import handlers.actions.user.ServitorMode;
import handlers.actions.user.ServitorMove;
import handlers.actions.user.ServitorSkillUse;
import handlers.actions.user.ServitorStop;
import handlers.actions.user.SitStand;
import handlers.actions.user.SocialAction;
import handlers.actions.user.TacticalSignTarget;
import handlers.actions.user.TacticalSignUse;
import handlers.actions.user.TeleportBookmark;
import handlers.actions.user.UnsummonPet;
import handlers.actions.user.UnsummonServitor;
import handlers.bypass.communityboard.ClanBoard;
import handlers.bypass.communityboard.DropSearchBoard;
import handlers.bypass.communityboard.FavoriteBoard;
import handlers.bypass.communityboard.FriendsBoard;
import handlers.bypass.communityboard.HomeBoard;
import handlers.bypass.communityboard.HomepageBoard;
import handlers.bypass.communityboard.MailBoard;
import handlers.bypass.communityboard.MemoBoard;
import handlers.bypass.communityboard.RegionBoard;
import handlers.bypass.npc.Augment;
import handlers.bypass.npc.Buy;
import handlers.bypass.npc.ChatLink;
import handlers.bypass.npc.ClanWarehouse;
import handlers.bypass.npc.EnsoulWindow;
import handlers.bypass.npc.FindPvP;
import handlers.bypass.npc.Freight;
import handlers.bypass.npc.ItemAuctionLink;
import handlers.bypass.npc.Link;
import handlers.bypass.npc.Multisell;
import handlers.bypass.npc.NpcViewMod;
import handlers.bypass.npc.Observation;
import handlers.bypass.npc.PlayerHelp;
import handlers.bypass.npc.PrivateWarehouse;
import handlers.bypass.npc.ReleaseAttribute;
import handlers.bypass.npc.ScriptLink;
import handlers.bypass.npc.SkillList;
import handlers.bypass.npc.TerritoryStatus;
import handlers.bypass.npc.TutorialClose;
import handlers.bypass.npc.VoiceCommand;
import handlers.bypass.npc.Wear;
import handlers.chat.channels.ChatAlliance;
import handlers.chat.channels.ChatClan;
import handlers.chat.channels.ChatGeneral;
import handlers.chat.channels.ChatHeroVoice;
import handlers.chat.channels.ChatParty;
import handlers.chat.channels.ChatPartyMatchRoom;
import handlers.chat.channels.ChatPartyRoomAll;
import handlers.chat.channels.ChatPartyRoomCommander;
import handlers.chat.channels.ChatPetition;
import handlers.chat.channels.ChatShout;
import handlers.chat.channels.ChatTrade;
import handlers.chat.channels.ChatWhisper;
import handlers.chat.channels.ChatWorld;
import handlers.chat.commands.admin.AdminAdmin;
import handlers.chat.commands.admin.AdminAnnouncements;
import handlers.chat.commands.admin.AdminAugment;
import handlers.chat.commands.admin.AdminBuffs;
import handlers.chat.commands.admin.AdminCamera;
import handlers.chat.commands.admin.AdminCastle;
import handlers.chat.commands.admin.AdminChangeAccessLevel;
import handlers.chat.commands.admin.AdminClan;
import handlers.chat.commands.admin.AdminClanHall;
import handlers.chat.commands.admin.AdminCreateItem;
import handlers.chat.commands.admin.AdminCursedWeapons;
import handlers.chat.commands.admin.AdminDebug;
import handlers.chat.commands.admin.AdminDelete;
import handlers.chat.commands.admin.AdminDestroyItems;
import handlers.chat.commands.admin.AdminDisconnect;
import handlers.chat.commands.admin.AdminDoorControl;
import handlers.chat.commands.admin.AdminEditChar;
import handlers.chat.commands.admin.AdminEffects;
import handlers.chat.commands.admin.AdminElement;
import handlers.chat.commands.admin.AdminEnchant;
import handlers.chat.commands.admin.AdminEvents;
import handlers.chat.commands.admin.AdminExpSp;
import handlers.chat.commands.admin.AdminFakePlayers;
import handlers.chat.commands.admin.AdminFence;
import handlers.chat.commands.admin.AdminFightCalculator;
import handlers.chat.commands.admin.AdminFortSiege;
import handlers.chat.commands.admin.AdminGeodata;
import handlers.chat.commands.admin.AdminGm;
import handlers.chat.commands.admin.AdminGmChat;
import handlers.chat.commands.admin.AdminGmSpeed;
import handlers.chat.commands.admin.AdminGoto;
import handlers.chat.commands.admin.AdminGraciaSeeds;
import handlers.chat.commands.admin.AdminGrandBoss;
import handlers.chat.commands.admin.AdminHeal;
import handlers.chat.commands.admin.AdminHelp;
import handlers.chat.commands.admin.AdminHide;
import handlers.chat.commands.admin.AdminHtml;
import handlers.chat.commands.admin.AdminHwid;
import handlers.chat.commands.admin.AdminInstance;
import handlers.chat.commands.admin.AdminInstanceZone;
import handlers.chat.commands.admin.AdminInvul;
import handlers.chat.commands.admin.AdminKick;
import handlers.chat.commands.admin.AdminKill;
import handlers.chat.commands.admin.AdminLevel;
import handlers.chat.commands.admin.AdminLogin;
import handlers.chat.commands.admin.AdminManor;
import handlers.chat.commands.admin.AdminMenu;
import handlers.chat.commands.admin.AdminMessages;
import handlers.chat.commands.admin.AdminMissingHtmls;
import handlers.chat.commands.admin.AdminOlympiad;
import handlers.chat.commands.admin.AdminOnline;
import handlers.chat.commands.admin.AdminPForge;
import handlers.chat.commands.admin.AdminPathNode;
import handlers.chat.commands.admin.AdminPcCafePoints;
import handlers.chat.commands.admin.AdminPetition;
import handlers.chat.commands.admin.AdminPledge;
import handlers.chat.commands.admin.AdminPremium;
import handlers.chat.commands.admin.AdminPrimePoints;
import handlers.chat.commands.admin.AdminPunishment;
import handlers.chat.commands.admin.AdminQuest;
import handlers.chat.commands.admin.AdminRegions;
import handlers.chat.commands.admin.AdminReload;
import handlers.chat.commands.admin.AdminRepairChar;
import handlers.chat.commands.admin.AdminRes;
import handlers.chat.commands.admin.AdminRide;
import handlers.chat.commands.admin.AdminScan;
import handlers.chat.commands.admin.AdminServerInfo;
import handlers.chat.commands.admin.AdminShop;
import handlers.chat.commands.admin.AdminShowQuests;
import handlers.chat.commands.admin.AdminShutdown;
import handlers.chat.commands.admin.AdminSkill;
import handlers.chat.commands.admin.AdminSpawn;
import handlers.chat.commands.admin.AdminSummon;
import handlers.chat.commands.admin.AdminSuperHaste;
import handlers.chat.commands.admin.AdminTarget;
import handlers.chat.commands.admin.AdminTargetSay;
import handlers.chat.commands.admin.AdminTeleport;
import handlers.chat.commands.admin.AdminTest;
import handlers.chat.commands.admin.AdminTransform;
import handlers.chat.commands.admin.AdminVitality;
import handlers.chat.commands.admin.AdminZone;
import handlers.chat.commands.admin.AdminZoneBuild;
import handlers.chat.commands.user.ChannelDelete;
import handlers.chat.commands.user.ChannelInfo;
import handlers.chat.commands.user.ChannelLeave;
import handlers.chat.commands.user.ClanPenalty;
import handlers.chat.commands.user.ClanWarsList;
import handlers.chat.commands.user.Dismount;
import handlers.chat.commands.user.InstanceZone;
import handlers.chat.commands.user.Loc;
import handlers.chat.commands.user.Mount;
import handlers.chat.commands.user.MyBirthday;
import handlers.chat.commands.user.OlympiadStat;
import handlers.chat.commands.user.PartyInfo;
import handlers.chat.commands.user.SiegeStatus;
import handlers.chat.commands.user.Time;
import handlers.chat.commands.user.Unstuck;
import handlers.chat.commands.voiced.AutoPlay;
import handlers.chat.commands.voiced.AutoPotion;
import handlers.chat.commands.voiced.Banking;
import handlers.chat.commands.voiced.ChangePassword;
import handlers.chat.commands.voiced.ChatAdmin;
import handlers.chat.commands.voiced.ExperienceGain;
import handlers.chat.commands.voiced.Lang;
import handlers.chat.commands.voiced.Offline;
import handlers.chat.commands.voiced.OfflinePlay;
import handlers.chat.commands.voiced.Online;
import handlers.chat.commands.voiced.Premium;
import handlers.chat.commands.voiced.Wedding;
import handlers.items.Appearance;
import handlers.items.BeastSoulShot;
import handlers.items.BeastSpiritShot;
import handlers.items.BlessedSoulShots;
import handlers.items.BlessedSpiritShot;
import handlers.items.Book;
import handlers.items.Bypass;
import handlers.items.Calculator;
import handlers.items.ChangeAttributeCrystal;
import handlers.items.CharmOfCourage;
import handlers.items.Elixir;
import handlers.items.EnchantAttribute;
import handlers.items.EnchantScrolls;
import handlers.items.EventItem;
import handlers.items.ExtractableItems;
import handlers.items.FatedSupportBox;
import handlers.items.FishShots;
import handlers.items.Harvester;
import handlers.items.ItemSkills;
import handlers.items.ItemSkillsTemplate;
import handlers.items.Maps;
import handlers.items.MercTicket;
import handlers.items.NicknameColor;
import handlers.items.PaulinasSupportBox;
import handlers.items.PetFood;
import handlers.items.Recipes;
import handlers.items.RollingDice;
import handlers.items.Seed;
import handlers.items.SoulShots;
import handlers.items.SpecialXMas;
import handlers.items.SpiritShot;
import handlers.items.SummonItems;
import handlers.punishments.BanHandler;
import handlers.punishments.ChatBanHandler;
import handlers.punishments.JailHandler;
import handlers.skill.targets.AdvanceBase;
import handlers.skill.targets.Artillery;
import handlers.skill.targets.DoorTreasure;
import handlers.skill.targets.Enemy;
import handlers.skill.targets.EnemyNot;
import handlers.skill.targets.EnemyOnly;
import handlers.skill.targets.FortressFlagpole;
import handlers.skill.targets.Ground;
import handlers.skill.targets.HolyThing;
import handlers.skill.targets.Item;
import handlers.skill.targets.MyMentor;
import handlers.skill.targets.MyParty;
import handlers.skill.targets.None;
import handlers.skill.targets.NpcBody;
import handlers.skill.targets.Others;
import handlers.skill.targets.OwnerPet;
import handlers.skill.targets.PcBody;
import handlers.skill.targets.Self;
import handlers.skill.targets.Summon;
import handlers.skill.targets.Target;
import handlers.skill.targets.WyvernTarget;
import handlers.skill.targets.affectobject.All;
import handlers.skill.targets.affectobject.Clan;
import handlers.skill.targets.affectobject.Friend;
import handlers.skill.targets.affectobject.FriendPc;
import handlers.skill.targets.affectobject.HiddenPlace;
import handlers.skill.targets.affectobject.Invisible;
import handlers.skill.targets.affectobject.NotFriend;
import handlers.skill.targets.affectobject.NotFriendPc;
import handlers.skill.targets.affectobject.ObjectDeadNpcBody;
import handlers.skill.targets.affectobject.UndeadRealEnemy;
import handlers.skill.targets.affectobject.WyvernObject;
import handlers.skill.targets.affectscope.DeadParty;
import handlers.skill.targets.affectscope.DeadPartyPledge;
import handlers.skill.targets.affectscope.DeadPledge;
import handlers.skill.targets.affectscope.DeadUnion;
import handlers.skill.targets.affectscope.Fan;
import handlers.skill.targets.affectscope.FanPB;
import handlers.skill.targets.affectscope.Party;
import handlers.skill.targets.affectscope.PartyPledge;
import handlers.skill.targets.affectscope.Pledge;
import handlers.skill.targets.affectscope.PointBlank;
import handlers.skill.targets.affectscope.Range;
import handlers.skill.targets.affectscope.RangeSortByHp;
import handlers.skill.targets.affectscope.RingRange;
import handlers.skill.targets.affectscope.Single;
import handlers.skill.targets.affectscope.Square;
import handlers.skill.targets.affectscope.SquarePB;
import handlers.skill.targets.affectscope.StaticObjectScope;
import handlers.skill.targets.affectscope.SummonExceptMaster;
import handlers.skill.targets.affectscope.ValakasScope;

/**
 * Master handler.
 * @author UnAfraid, Mobius
 */
public class MasterHandler
{
	private static final Logger LOGGER = Logger.getLogger(MasterHandler.class.getName());
	
	private static final IHandler<?, ?>[] LOAD_INSTANCES =
	{
		ActionClickHandler.getInstance(),
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
		ActionUserHandler.getInstance()
	};
	
	private static final Class<?>[][] HANDLERS =
	{
		{
			// Action Handlers
			ArtefactClick.class,
			DecoyClick.class,
			DoorClick.class,
			ItemClick.class,
			NpcClick.class,
			PlayerClick.class,
			PetClick.class,
			StaticObjectClick.class,
			SummonClick.class,
			TrapClick.class,
		},
		{
			// Action Shift Handlers
			DoorShiftClick.class,
			ItemShiftClick.class,
			NpcShiftClick.class,
			PlayerShiftClick.class,
			StaticObjectShiftClick.class,
			SummonShiftClick.class,
		},
		{
			// Admin Command Handlers
			AdminAdmin.class,
			AdminAnnouncements.class,
			AdminAugment.class,
			AdminBuffs.class,
			AdminCamera.class,
			AdminChangeAccessLevel.class,
			AdminClan.class,
			AdminClanHall.class,
			AdminCastle.class,
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
			AdminGoto.class,
			AdminGraciaSeeds.class,
			AdminGrandBoss.class,
			AdminHeal.class,
			AdminHelp.class,
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
			AdminOlympiad.class,
			AdminOnline.class,
			AdminPathNode.class,
			AdminPcCafePoints.class,
			AdminPetition.class,
			AdminPForge.class,
			AdminPledge.class,
			AdminPremium.class,
			AdminPrimePoints.class,
			AdminPunishment.class,
			AdminQuest.class,
			AdminRegions.class,
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
			AdminZoneBuild.class,
		},
		{
			// Bypass Handlers
			Augment.class,
			Buy.class,
			ChatLink.class,
			ClanWarehouse.class,
			EnsoulWindow.class,
			FindPvP.class,
			Freight.class,
			ItemAuctionLink.class,
			Link.class,
			Multisell.class,
			NpcViewMod.class,
			Observation.class,
			PlayerHelp.class,
			PrivateWarehouse.class,
			ReleaseAttribute.class,
			ScriptLink.class,
			SkillList.class,
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
			EventItem.class,
			ExtractableItems.class,
			FatedSupportBox.class,
			FishShots.class,
			Harvester.class,
			ItemSkills.class,
			ItemSkillsTemplate.class,
			Maps.class,
			MercTicket.class,
			NicknameColor.class,
			PaulinasSupportBox.class,
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
			// TODO: Add configuration options for this voiced commands.
			// CastleHandler.class,
			// ClanHandler.class,
			ExperienceGain.class,
			WeddingConfig.ALLOW_WEDDING ? Wedding.class : null,
			AutoPlayConfig.ENABLE_AUTO_PLAY ? AutoPlay.class : null,
			BankingConfig.BANKING_SYSTEM_ENABLED ? Banking.class : null,
			ChatModerationConfig.CHAT_ADMIN ? ChatAdmin.class : null,
			MultilingualSupportConfig.MULTILANG_ENABLE && MultilingualSupportConfig.MULTILANG_VOICED_ALLOW ? Lang.class : null,
			PasswordChangeConfig.ALLOW_CHANGE_PASSWORD ? ChangePassword.class : null,
			OfflinePlayConfig.ENABLE_OFFLINE_PLAY_COMMAND ? OfflinePlay.class : null,
			OfflineTradeConfig.ENABLE_OFFLINE_COMMAND && (OfflineTradeConfig.OFFLINE_TRADE_ENABLE || OfflineTradeConfig.OFFLINE_CRAFT_ENABLE) ? Offline.class : null,
			OnlineInfoConfig.ENABLE_ONLINE_COMMAND ? Online.class : null,
			PremiumSystemConfig.PREMIUM_SYSTEM_ENABLED ? Premium.class : null,
			AutoPotionsConfig.AUTO_POTIONS_ENABLED ? AutoPotion.class : null,
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
			ValakasScope.class,
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
