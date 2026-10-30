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
package handlers.bypass.npc;

import java.text.DecimalFormat;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.StringTokenizer;
import java.util.concurrent.TimeUnit;

import org.l2jmobius.commons.util.StringUtil;
import org.l2jmobius.gameserver.cache.HtmCache;
import org.l2jmobius.gameserver.config.NpcConfig;
import org.l2jmobius.gameserver.config.RatesConfig;
import org.l2jmobius.gameserver.config.custom.PremiumSystemConfig;
import org.l2jmobius.gameserver.data.xml.ItemData;
import org.l2jmobius.gameserver.handler.IBypassHandler;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.AttributeType;
import org.l2jmobius.gameserver.model.actor.enums.npc.DropType;
import org.l2jmobius.gameserver.model.actor.holders.npc.DropGroupHolder;
import org.l2jmobius.gameserver.model.actor.holders.npc.DropHolder;
import org.l2jmobius.gameserver.model.actor.stat.PlayerStat;
import org.l2jmobius.gameserver.model.item.ItemTemplate;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.spawns.Spawn;
import org.l2jmobius.gameserver.model.stats.Stat;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;
import org.l2jmobius.gameserver.util.HtmlUtil;

/**
 * @author NosBit, Mobius
 */
public class NpcViewMod implements IBypassHandler
{
	private static final String[] COMMANDS =
	{
		"NpcViewMod"
	};
	
	private static final int DROP_LIST_ITEMS_PER_PAGE = 10;
	
	@Override
	public boolean onCommand(String command, Player player, Creature bypassOrigin)
	{
		final StringTokenizer st = new StringTokenizer(command);
		st.nextToken();
		
		if (!st.hasMoreTokens())
		{
			LOGGER.warning("Bypass[NpcViewMod] used without enough parameters.");
			return false;
		}
		
		final String actualCommand = st.nextToken();
		switch (actualCommand.toLowerCase())
		{
			case "view":
			{
				final WorldObject target;
				if (st.hasMoreElements())
				{
					try
					{
						target = World.getInstance().findObject(Integer.parseInt(st.nextToken()));
					}
					catch (NumberFormatException e)
					{
						return false;
					}
				}
				else
				{
					target = player.getTarget();
				}
				
				final Npc npc = target instanceof Npc ? target.asNpc() : null;
				if (npc == null)
				{
					return false;
				}
				
				sendNpcView(player, npc);
				break;
			}
			case "droplist":
			{
				if (st.countTokens() < 2)
				{
					LOGGER.warning("Bypass[NpcViewMod] used without enough parameters.");
					return false;
				}
				
				final String dropListTypeString = st.nextToken();
				try
				{
					final DropType dropListType = Enum.valueOf(DropType.class, dropListTypeString);
					final WorldObject target = World.getInstance().findObject(Integer.parseInt(st.nextToken()));
					final Npc npc = target instanceof Npc ? target.asNpc() : null;
					if (npc == null)
					{
						return false;
					}
					
					final int page = st.hasMoreElements() ? Integer.parseInt(st.nextToken()) : 0;
					sendNpcDropList(player, npc, dropListType, page);
				}
				catch (NumberFormatException e)
				{
					return false;
				}
				catch (IllegalArgumentException e)
				{
					LOGGER.warning("Bypass[NpcViewMod] unknown drop list scope: " + dropListTypeString);
					return false;
				}
				break;
			}
			case "skills":
			{
				final WorldObject target;
				if (st.hasMoreElements())
				{
					try
					{
						target = World.getInstance().findObject(Integer.parseInt(st.nextToken()));
					}
					catch (NumberFormatException e)
					{
						return false;
					}
				}
				else
				{
					target = player.getTarget();
				}
				
				final Npc npc = target instanceof Npc ? target.asNpc() : null;
				if (npc == null)
				{
					return false;
				}
				
				sendNpcSkillView(player, npc);
				break;
			}
			case "aggrolist":
			{
				final WorldObject target;
				if (st.hasMoreElements())
				{
					try
					{
						target = World.getInstance().findObject(Integer.parseInt(st.nextToken()));
					}
					catch (NumberFormatException e)
					{
						return false;
					}
				}
				else
				{
					target = player.getTarget();
				}
				
				final Npc npc = target instanceof Npc ? target.asNpc() : null;
				if (npc == null)
				{
					return false;
				}
				
				sendAggroListView(player, npc);
				break;
			}
		}
		
		return true;
	}
	
	@Override
	public String[] getCommandList()
	{
		return COMMANDS;
	}
	
	public static void sendNpcView(Player player, Npc npc)
	{
		final NpcHtmlMessage html = new NpcHtmlMessage();
		html.setFile(player, "data/html/mods/NpcView/Info.htm");
		html.replace("%name%", npc.getName());
		html.replace("%hpGauge%", HtmlUtil.getHpGauge(250, (long) npc.getCurrentHp(), npc.getMaxHp(), false));
		html.replace("%mpGauge%", HtmlUtil.getMpGauge(250, (long) npc.getCurrentMp(), npc.getMaxMp(), false));
		
		final Spawn npcSpawn = npc.getSpawn();
		if ((npcSpawn == null) || (npcSpawn.getRespawnMinDelay() == 0))
		{
			html.replace("%respawn%", "None");
		}
		else
		{
			long minRespawnDelay = (long) (npcSpawn.getRespawnMinDelay() * NpcConfig.RAID_MIN_RESPAWN_MULTIPLIER);
			long maxRespawnDelay = (long) (npcSpawn.getRespawnMaxDelay() * NpcConfig.RAID_MAX_RESPAWN_MULTIPLIER);
			TimeUnit timeUnit = TimeUnit.MILLISECONDS;
			
			long min = Long.MAX_VALUE;
			for (TimeUnit tu : TimeUnit.values())
			{
				final long minTimeFromMillis = tu.convert(npcSpawn.getRespawnMinDelay(), TimeUnit.MILLISECONDS);
				final long maxTimeFromMillis = tu.convert(npcSpawn.getRespawnMaxDelay(), TimeUnit.MILLISECONDS);
				if ((TimeUnit.MILLISECONDS.convert(minTimeFromMillis, tu) == npcSpawn.getRespawnMinDelay()) && (TimeUnit.MILLISECONDS.convert(maxTimeFromMillis, tu) == npcSpawn.getRespawnMaxDelay()) && (min > minTimeFromMillis))
				{
					min = minTimeFromMillis;
					timeUnit = tu;
				}
			}
			
			minRespawnDelay = timeUnit.convert(minRespawnDelay, TimeUnit.MILLISECONDS);
			maxRespawnDelay = timeUnit.convert(maxRespawnDelay, TimeUnit.MILLISECONDS);
			
			final String timeUnitName = timeUnit.name().charAt(0) + timeUnit.name().toLowerCase().substring(1);
			if (npcSpawn.hasRespawnRandom())
			{
				html.replace("%respawn%", minRespawnDelay + "-" + maxRespawnDelay + " " + timeUnitName);
			}
			else
			{
				html.replace("%respawn%", minRespawnDelay + " " + timeUnitName);
			}
		}
		
		html.replace("%atktype%", StringUtil.capitalizeFirst(npc.getAttackType().name().toLowerCase()));
		html.replace("%atkrange%", npc.getStat().getPhysicalAttackRange());
		html.replace("%patk%", npc.getPAtk());
		html.replace("%pdef%", npc.getPDef());
		html.replace("%matk%", npc.getMAtk());
		html.replace("%mdef%", npc.getMDef());
		html.replace("%atkspd%", npc.getPAtkSpd());
		html.replace("%castspd%", npc.getMAtkSpd());
		html.replace("%critrate%", npc.getStat().getCriticalHit());
		html.replace("%evasion%", npc.getEvasionRate());
		html.replace("%accuracy%", npc.getStat().getAccuracy());
		html.replace("%speed%", (int) npc.getStat().getMoveSpeed());
		html.replace("%attributeatktype%", npc.getStat().getAttackElement().name());
		html.replace("%attributeatkvalue%", npc.getStat().getAttackElementValue(npc.getStat().getAttackElement()));
		html.replace("%attributefire%", npc.getStat().getDefenseElementValue(AttributeType.FIRE));
		html.replace("%attributewater%", npc.getStat().getDefenseElementValue(AttributeType.WATER));
		html.replace("%attributewind%", npc.getStat().getDefenseElementValue(AttributeType.WIND));
		html.replace("%attributeearth%", npc.getStat().getDefenseElementValue(AttributeType.EARTH));
		html.replace("%attributedark%", npc.getStat().getDefenseElementValue(AttributeType.DARK));
		html.replace("%attributeholy%", npc.getStat().getDefenseElementValue(AttributeType.HOLY));
		html.replace("%dropListButtons%", getDropListButtons(npc));
		player.sendPacket(html);
	}
	
	private void sendNpcSkillView(Player player, Npc npc)
	{
		final NpcHtmlMessage html = new NpcHtmlMessage();
		html.setFile(player, "data/html/mods/NpcView/Skills.htm");
		
		final StringBuilder sb = new StringBuilder();
		npc.getSkills().values().forEach(s ->
		{
			sb.append("<table width=277 height=32 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">");
			sb.append("<tr><td width=32>");
			sb.append("<img src=\"");
			sb.append(s.getIcon());
			sb.append("\" width=32 height=32>");
			sb.append("</td><td width=110>");
			sb.append(s.getName());
			sb.append("</td>");
			sb.append("<td width=45 align=center>");
			sb.append(s.getId());
			sb.append("</td>");
			sb.append("<td width=35 align=center>");
			sb.append(s.getLevel());
			sb.append("</td></tr></table>");
		});
		
		html.replace("%skills%", sb.toString());
		html.replace("%npc_name%", npc.getName());
		html.replace("%npcId%", npc.getId());
		player.sendPacket(html);
	}
	
	private void sendAggroListView(Player player, Npc npc)
	{
		final NpcHtmlMessage html = new NpcHtmlMessage();
		html.setFile(player, "data/html/mods/NpcView/AggroList.htm");
		
		final StringBuilder sb = new StringBuilder();
		if (npc.isAttackable())
		{
			npc.asAttackable().getAggroList().values().forEach(a ->
			{
				sb.append("<table width=277 height=32 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">");
				sb.append("<tr><td width=110>");
				sb.append(a.getAttacker() != null ? a.getAttacker().getName() : "NULL");
				sb.append("</td>");
				sb.append("<td width=60 align=center>");
				sb.append(a.getHate());
				sb.append("</td>");
				sb.append("<td width=60 align=center>");
				sb.append(a.getDamage());
				sb.append("</td></tr></table>");
			});
		}
		
		html.replace("%aggrolist%", sb.toString());
		html.replace("%npc_name%", npc.getName());
		html.replace("%npcId%", npc.getId());
		html.replace("%objid%", npc.getObjectId());
		player.sendPacket(html);
	}
	
	private static String getDropListButtons(Npc npc)
	{
		final StringBuilder sb = new StringBuilder();
		final List<DropGroupHolder> dropListGroups = npc.getTemplate().getDropGroups();
		final List<DropHolder> dropListDeath = npc.getTemplate().getDropList();
		final List<DropHolder> dropListSpoil = npc.getTemplate().getSpoilList();
		if ((dropListGroups != null) || (dropListDeath != null) || (dropListSpoil != null))
		{
			sb.append("<table width=275 cellpadding=0 cellspacing=0><tr>");
			if ((dropListGroups != null) || (dropListDeath != null))
			{
				sb.append("<td align=center><button value=\"Show Drop\" width=100 height=25 action=\"bypass NpcViewMod dropList DROP " + npc.getObjectId() + "\" back=\"L2UI_CT1.Button_DF_Calculator_Down\" fore=\"L2UI_CT1.Button_DF_Calculator\"></td>");
			}
			
			if (dropListSpoil != null)
			{
				sb.append("<td align=center><button value=\"Show Spoil\" width=100 height=25 action=\"bypass NpcViewMod dropList SPOIL " + npc.getObjectId() + "\" back=\"L2UI_CT1.Button_DF_Calculator_Down\" fore=\"L2UI_CT1.Button_DF_Calculator\"></td>");
			}
			
			sb.append("</tr></table>");
		}
		
		return sb.toString();
	}
	
	private void sendNpcDropList(Player player, Npc npc, DropType dropType, int pageValue)
	{
		List<DropHolder> dropList = null;
		if (dropType == DropType.SPOIL)
		{
			dropList = new ArrayList<>(npc.getTemplate().getSpoilList());
		}
		else
		{
			final List<DropHolder> drops = npc.getTemplate().getDropList();
			if (drops != null)
			{
				dropList = new ArrayList<>(drops);
			}
			
			final List<DropGroupHolder> dropGroups = npc.getTemplate().getDropGroups();
			if (dropGroups != null)
			{
				if (dropList == null)
				{
					dropList = new ArrayList<>();
				}
				
				for (DropGroupHolder dropGroup : dropGroups)
				{
					final double chance = dropGroup.getChance() / 100;
					for (DropHolder dropHolder : dropGroup.getDropList())
					{
						dropList.add(new DropHolder(dropHolder.getDropType(), dropHolder.getItemId(), dropHolder.getMin(), dropHolder.getMax(), dropHolder.getChance() * chance));
					}
				}
			}
		}
		
		if (dropList == null)
		{
			return;
		}
		
		Collections.sort(dropList, (d1, d2) -> Integer.valueOf(d1.getItemId()).compareTo(Integer.valueOf(d2.getItemId())));
		
		int pages = dropList.size() / DROP_LIST_ITEMS_PER_PAGE;
		if ((DROP_LIST_ITEMS_PER_PAGE * pages) < dropList.size())
		{
			pages++;
		}
		
		final StringBuilder pagesSb = new StringBuilder();
		if (pages > 1)
		{
			pagesSb.append("<table><tr>");
			for (int i = 0; i < pages; i++)
			{
				pagesSb.append("<td align=center><button value=\"" + (i + 1) + "\" width=20 height=20 action=\"bypass NpcViewMod dropList " + dropType + " " + npc.getObjectId() + " " + i + "\" back=\"L2UI_CT1.Button_DF_Calculator_Down\" fore=\"L2UI_CT1.Button_DF_Calculator\"></td>");
			}
			pagesSb.append("</tr></table>");
		}
		
		int page = pageValue;
		if (page >= pages)
		{
			page = pages - 1;
		}
		
		final int start = page > 0 ? page * DROP_LIST_ITEMS_PER_PAGE : 0;
		int end = (page * DROP_LIST_ITEMS_PER_PAGE) + DROP_LIST_ITEMS_PER_PAGE;
		if (end > dropList.size())
		{
			end = dropList.size();
		}
		
		final DecimalFormat amountFormat = new DecimalFormat("#,###");
		final DecimalFormat chanceFormat = new DecimalFormat("0.00##");
		int leftHeight = 0;
		int rightHeight = 0;
		final PlayerStat stat = player.getStat();
		final double dropAmountAdenaEffectBonus = stat.getMul(Stat.BONUS_DROP_ADENA, 1);
		final double dropAmountEffectBonus = stat.getMul(Stat.BONUS_DROP_AMOUNT, 1);
		final double dropRateEffectBonus = stat.getMul(Stat.BONUS_DROP_RATE, 1);
		final double spoilRateEffectBonus = stat.getMul(Stat.BONUS_SPOIL_RATE, 1);
		final StringBuilder leftSb = new StringBuilder();
		final StringBuilder rightSb = new StringBuilder();
		String limitReachedMsg = "";
		for (int i = start; i < end; i++)
		{
			final StringBuilder sb = new StringBuilder();
			final int height = 64;
			final DropHolder dropItem = dropList.get(i);
			final ItemTemplate item = ItemData.getInstance().getTemplate(dropItem.getItemId());
			
			// real time server rate calculations
			double rateChance = 1;
			double rateAmount = 1;
			if (dropType == DropType.SPOIL)
			{
				rateChance = RatesConfig.RATE_SPOIL_DROP_CHANCE_MULTIPLIER;
				rateAmount = RatesConfig.RATE_SPOIL_DROP_AMOUNT_MULTIPLIER;
				
				// also check premium rates if available
				if (PremiumSystemConfig.PREMIUM_SYSTEM_ENABLED && player.hasPremiumStatus())
				{
					rateChance *= PremiumSystemConfig.PREMIUM_RATE_SPOIL_CHANCE;
					rateAmount *= PremiumSystemConfig.PREMIUM_RATE_SPOIL_AMOUNT;
				}
				
				// bonus spoil rate effect
				rateChance *= spoilRateEffectBonus;
			}
			else
			{
				if (RatesConfig.RATE_DROP_CHANCE_BY_ID.get(dropItem.getItemId()) != null)
				{
					rateChance *= RatesConfig.RATE_DROP_CHANCE_BY_ID.get(dropItem.getItemId());
					if ((dropItem.getItemId() == Inventory.ADENA_ID) && (rateChance > 100))
					{
						rateChance = 100;
					}
				}
				else if (item.hasExImmediateEffect())
				{
					rateChance *= RatesConfig.RATE_HERB_DROP_CHANCE_MULTIPLIER;
				}
				else if (npc.isRaid())
				{
					rateChance *= RatesConfig.RATE_RAID_DROP_CHANCE_MULTIPLIER;
				}
				else
				{
					rateChance *= RatesConfig.RATE_DEATH_DROP_CHANCE_MULTIPLIER;
				}
				
				if (RatesConfig.RATE_DROP_AMOUNT_BY_ID.get(dropItem.getItemId()) != null)
				{
					rateAmount *= RatesConfig.RATE_DROP_AMOUNT_BY_ID.get(dropItem.getItemId());
				}
				else if (item.hasExImmediateEffect())
				{
					rateAmount *= RatesConfig.RATE_HERB_DROP_AMOUNT_MULTIPLIER;
				}
				else if (npc.isRaid())
				{
					rateAmount *= RatesConfig.RATE_RAID_DROP_AMOUNT_MULTIPLIER;
				}
				else
				{
					rateAmount *= RatesConfig.RATE_DEATH_DROP_AMOUNT_MULTIPLIER;
				}
				
				// also check premium rates if available
				if (PremiumSystemConfig.PREMIUM_SYSTEM_ENABLED && player.hasPremiumStatus())
				{
					if (PremiumSystemConfig.PREMIUM_RATE_DROP_CHANCE_BY_ID.get(dropItem.getItemId()) != null)
					{
						rateChance *= PremiumSystemConfig.PREMIUM_RATE_DROP_CHANCE_BY_ID.get(dropItem.getItemId());
					}
					else if (item.hasExImmediateEffect())
					{
						// TODO: Premium herb chance? :)
					}
					else if (npc.isRaid())
					{
						// TODO: Premium raid chance? :)
					}
					else
					{
						rateChance *= PremiumSystemConfig.PREMIUM_RATE_DROP_CHANCE;
					}
					
					if (PremiumSystemConfig.PREMIUM_RATE_DROP_AMOUNT_BY_ID.get(dropItem.getItemId()) != null)
					{
						rateAmount *= PremiumSystemConfig.PREMIUM_RATE_DROP_AMOUNT_BY_ID.get(dropItem.getItemId());
					}
					else if (item.hasExImmediateEffect())
					{
						// TODO: Premium herb amount? :)
					}
					else if (npc.isRaid())
					{
						// TODO: Premium raid amount? :)
					}
					else
					{
						rateAmount *= PremiumSystemConfig.PREMIUM_RATE_DROP_AMOUNT;
					}
				}
				
				// bonus drop amount effect
				rateAmount *= dropAmountEffectBonus;
				if (item.getId() == Inventory.ADENA_ID)
				{
					rateAmount *= dropAmountAdenaEffectBonus;
				}
				
				// bonus drop rate effect
				rateChance *= dropRateEffectBonus;
			}
			
			// Do not display zero chance drops.
			if (rateChance == 0d)
			{
				continue;
			}
			
			sb.append("<table width=332 cellpadding=2 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">");
			sb.append("<tr><td width=32 valign=top>");
			sb.append("<button width=\"32\" height=\"32\" back=\"" + (item.getIcon() == null ? "icon.etc_question_mark_i00" : item.getIcon()) + "\" fore=\"" + (item.getIcon() == null ? "icon.etc_question_mark_i00" : item.getIcon()) + "\" itemtooltip=\"" + dropItem.getItemId() + "\">");
			sb.append("</td><td fixwidth=300 align=center><font name=\"hs9\" color=\"CD9000\">");
			sb.append(item.getName());
			sb.append("</font></td></tr><tr><td width=32></td><td width=300><table width=295 cellpadding=0 cellspacing=0>");
			sb.append("<tr><td width=48 align=right valign=top><font color=\"LEVEL\">Amount:</font></td>");
			sb.append("<td width=247 align=center>");
			
			final long min = (long) (dropItem.getMin() * rateAmount);
			final long max = (long) (dropItem.getMax() * rateAmount);
			if (min == max)
			{
				sb.append(amountFormat.format(min));
			}
			else
			{
				sb.append(amountFormat.format(min));
				sb.append(" - ");
				sb.append(amountFormat.format(max));
			}
			
			sb.append("</td></tr><tr><td width=48 align=right valign=top><font color=\"LEVEL\">Chance:</font></td>");
			sb.append("<td width=247 align=center>");
			sb.append(chanceFormat.format(Math.min(dropItem.getChance() * rateChance, 100)));
			sb.append("%</td></tr></table></td></tr><tr><td width=32></td><td width=300>&nbsp;</td></tr></table>");
			if ((sb.length() + rightSb.length() + leftSb.length()) < 16000) // limit of 32766?
			{
				if (leftHeight >= (rightHeight + height))
				{
					rightSb.append(sb);
					rightHeight += height;
				}
				else
				{
					leftSb.append(sb);
					leftHeight += height;
				}
			}
			else
			{
				limitReachedMsg = "<br><center>Too many drops! Could not display them all!</center>";
			}
		}
		
		final StringBuilder bodySb = new StringBuilder();
		bodySb.append("<table><tr>");
		bodySb.append("<td>");
		bodySb.append(leftSb.toString());
		bodySb.append("</td><td>");
		bodySb.append(rightSb.toString());
		bodySb.append("</td>");
		bodySb.append("</tr></table>");
		
		String html = HtmCache.getInstance().getHtm(player, "data/html/mods/NpcView/DropList.htm");
		if (html == null)
		{
			LOGGER.warning(NpcViewMod.class.getSimpleName() + ": The html file data/html/mods/NpcView/DropList.htm could not be found.");
			return;
		}
		
		html = html.replace("%name%", npc.getName());
		html = html.replace("%dropListButtons%", getDropListButtons(npc));
		html = html.replace("%pages%", pagesSb.toString());
		html = html.replace("%items%", bodySb.toString() + limitReachedMsg);
		HtmlUtil.sendCBHtml(player, html);
	}
}
