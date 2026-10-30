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
package custom.events.Rabbits;

import java.io.File;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

import org.w3c.dom.Document;
import org.w3c.dom.Node;

import org.l2jmobius.commons.time.SchedulingPattern;
import org.l2jmobius.commons.time.TimeUtil;
import org.l2jmobius.commons.util.IXmlReader;
import org.l2jmobius.gameserver.config.GeneralConfig;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Event;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.util.Broadcast;

/**
 * Rabbits event.<br>
 * Chests are hidden at Fantasy Isle and players must use the Rabbit transformation's skills to find and open them.
 * @author Gnacik, Zoey76
 */
public class Rabbits extends Event
{
	// NPCs
	private static final int NPC_MANAGER = 900101;
	private static final int CHEST = 900102;
	
	// Skills
	private static final SkillHolder RABBIT_MAGIC_EYE = new SkillHolder(629, 1);
	private static final SkillHolder RABBIT_TORNADO = new SkillHolder(630, 1);
	private static final SkillHolder RABBIT_TRANSFORMATION = new SkillHolder(2428, 1);
	private static final SkillHolder RAID_CURSE = new SkillHolder(4515, 1);
	
	// Misc
	private static final int EVENT_TIME = 10;
	private static final int TOTAL_CHEST_COUNT = 75;
	private static final int TRANSFORMATION_ID = 105;
	private final Collection<Npc> _npcs = ConcurrentHashMap.newKeySet();
	private final List<Player> _players = new ArrayList<>();
	private boolean _isActive = false;
	
	/**
	 * Drop data:<br>
	 * Higher the chance harder the item.<br>
	 * ItemId, chance in percent, min amount, max amount
	 */
	// @formatter:off
	private static final int[][] DROPLIST =
	{
		{  1540,  80, 10, 15 },	// Quick Healing Potion
		{  1538,  60,  5, 10 },	// Blessed Scroll of Escape
		{  3936,  40,  5, 10 },	// Blessed Scroll of Ressurection
		{  6387,  25,  5, 10 },	// Blessed Scroll of Ressurection Pets
		{ 22025,  15,  5, 10 },	// Powerful Healing Potion
		{  6622,  10,  1, 1 },	// Giant's Codex
		{ 20034,   5,  1, 1 },	// Revita Pop
		{ 20004,   1,  1, 1 },	// Energy Ginseng
		{ 20004,   0,  1, 1 }	// Energy Ginseng
	};
	// @formatter:on
	
	private Rabbits()
	{
		addFirstTalkId(NPC_MANAGER, CHEST);
		addTalkId(NPC_MANAGER);
		addStartNpc(NPC_MANAGER);
		addSkillSeeId(CHEST);
		addAttackId(CHEST);
		
		loadConfig();
	}
	
	private void loadConfig()
	{
		new IXmlReader()
		{
			@Override
			public void load()
			{
				parseDatapackFile("data/scripts/custom/events/Rabbits/config.xml");
			}
			
			@Override
			public void parseDocument(Document document, File file)
			{
				final AtomicInteger count = new AtomicInteger(0);
				forEach(document, "event", eventNode ->
				{
					final StatSet att = new StatSet(parseAttributes(eventNode));
					final String name = att.getString("name");
					for (Node node = document.getDocumentElement().getFirstChild(); node != null; node = node.getNextSibling())
					{
						switch (node.getNodeName())
						{
							case "schedule":
							{
								final StatSet attributes = new StatSet(parseAttributes(node));
								final String pattern = attributes.getString("pattern");
								final SchedulingPattern schedulingPattern = new SchedulingPattern(pattern);
								final StatSet params = new StatSet();
								params.set("Name", name);
								params.set("SchedulingPattern", pattern);
								final long delay = schedulingPattern.getDelayToNextFromNow();
								getTimers().addTimer("Schedule" + count.incrementAndGet(), params, delay + 5000, null, null); // Added 5 seconds to prevent overlapping.
								LOGGER.info("Event " + name + " scheduled at " + TimeUtil.getDateTimeString(System.currentTimeMillis() + delay));
								break;
							}
						}
					}
				});
			}
		}.load();
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		if (event.startsWith("Schedule"))
		{
			eventStart(null);
			final SchedulingPattern schedulingPattern = new SchedulingPattern(params.getString("SchedulingPattern"));
			final long delay = schedulingPattern.getDelayToNextFromNow();
			getTimers().addTimer(event, params, delay + 5000, null, null); // Added 5 seconds to prevent overlapping.
			LOGGER.info("Event " + params.getString("Name") + " scheduled at " + TimeUtil.getDateTimeString(System.currentTimeMillis() + delay));
		}
	}
	
	@Override
	public boolean eventStart(Player eventMaker)
	{
		// Don't start event if it is active
		if (_isActive)
		{
			eventMaker.sendMessage("Event " + getName() + " is already started!");
			return false;
		}
		
		// Check starting conditions
		if (!GeneralConfig.CUSTOM_NPC_DATA)
		{
			LOGGER.info(getName() + ": Event can't be started, because custom NPCs are disabled!");
			eventMaker.sendMessage("Event " + getName() + " can't be started because custom NPCs are disabled!");
			return false;
		}
		
		// Set Event active
		_isActive = true;
		
		// Spawn Manager
		recordSpawn(_npcs, NPC_MANAGER, -59227, -56939, -2039, 64106, false, 0);
		
		// Spawn Chests
		for (int i = 0; i <= TOTAL_CHEST_COUNT; i++)
		{
			recordSpawn(_npcs, CHEST, getRandom(-60653, -58772), getRandom(-55830, -58146), -2030, 0, false, EVENT_TIME * 60000);
		}
		
		// Announce event start
		Broadcast.toAllOnlinePlayers("Rabbits Event: Chests spawned!");
		Broadcast.toAllOnlinePlayers("Rabbits Event: Go to Fantasy Isle and grab some rewards!");
		Broadcast.toAllOnlinePlayers("Rabbits Event: You have " + EVENT_TIME + " minuntes!");
		Broadcast.toAllOnlinePlayers("Rabbits Event: After that time all chests will disappear...");
		
		// Schedule event end
		startQuestTimer("END_RABBITS_EVENT", EVENT_TIME * 60000, null, eventMaker);
		return true;
	}
	
	@Override
	public boolean eventStop()
	{
		// Don't stop inactive event
		if (!_isActive)
		{
			return false;
		}
		
		// Set inactive
		_isActive = false;
		
		// Cancel timer
		cancelQuestTimers("END_RABBITS_EVENT");
		
		// Despawn NPCs
		for (Npc npc : _npcs)
		{
			if (npc != null)
			{
				npc.deleteMe();
			}
		}
		
		_npcs.clear();
		
		for (Player player : _players)
		{
			if ((player != null) && (player.getTransformationId() == TRANSFORMATION_ID))
			{
				player.untransform();
			}
		}
		
		_players.clear();
		
		// Announce event end
		Broadcast.toAllOnlinePlayers("Rabbits Event: Event has finished.");
		return true;
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "900101-1.htm":
			{
				htmltext = "900101-1.htm";
				break;
			}
			case "transform":
			{
				if (player.isTransformed())
				{
					player.untransform();
				}
				
				if (player.isSitting())
				{
					player.standUp();
				}
				
				RABBIT_TRANSFORMATION.getSkill().applyEffects(player, player);
				_players.add(player);
				break;
			}
			case "END_RABBITS_EVENT":
			{
				Broadcast.toAllOnlinePlayers("Rabbits Event: Time up!");
				eventStop();
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".htm";
	}
	
	@Override
	public void onSkillSee(Npc npc, Player caster, Skill skill, Collection<WorldObject> targets, boolean isSummon)
	{
		if (skill.getId() == RABBIT_TORNADO.getSkillId())
		{
			if (!npc.isInvisible() && targets.contains(npc))
			{
				dropItem(npc, caster, DROPLIST);
				npc.deleteMe();
				_npcs.remove(npc);
				
				if (_npcs.size() <= 1)
				{
					Broadcast.toAllOnlinePlayers("Rabbits Event: No more chests...");
					eventStop();
				}
			}
		}
		else if ((skill.getId() == RABBIT_MAGIC_EYE.getSkillId()) && npc.isInvisible() && npc.isInsideRadius2D(caster, skill.getAffectRange()))
		{
			npc.setInvisible(false);
		}
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon, Skill skill)
	{
		if (_isActive && ((skill == null) || (skill.getId() != RABBIT_TORNADO.getSkillId())))
		{
			RAID_CURSE.getSkill().applyEffects(npc, attacker);
		}
	}
	
	private void dropItem(Npc npc, Player player, int[][] droplist)
	{
		final int chance = getRandom(100);
		for (int[] drop : droplist)
		{
			if (chance > drop[1])
			{
				npc.dropItem(player, drop[0], getRandom(drop[2], drop[3]));
				return;
			}
		}
	}
	
	private void recordSpawn(Collection<Npc> npcs, int npcId, int x, int y, int z, int heading, boolean randomOffSet, long despawnDelay)
	{
		final Npc npc = addSpawn(npcId, x, y, z, heading, randomOffSet, despawnDelay);
		if (npc.getId() == CHEST)
		{
			npc.setImmobilized(true);
			npc.disableCoreAI(true);
			npc.setInvisible(true);
		}
		
		npcs.add(npc);
	}
	
	@Override
	public boolean eventBypass(Player player, String bypass)
	{
		return false;
	}
	
	public static void main(String[] args)
	{
		new Rabbits();
	}
}
