DROP TABLE IF EXISTS `npc_respawns`;
CREATE TABLE IF NOT EXISTS `npc_respawns` (
  `id` int NOT NULL,
  `x` int NOT NULL,
  `y` int NOT NULL,
  `z` int NOT NULL,
  `heading` int NOT NULL,
  `respawnTime` bigint NOT NULL DEFAULT '0',
  `currentHp` double NOT NULL,
  `currentMp` double NOT NULL,
  PRIMARY KEY (`id`)
) ;