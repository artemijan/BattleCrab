DROP TABLE IF EXISTS `fort_siege_guards`;
CREATE TABLE IF NOT EXISTS `fort_siege_guards` (
  `fortId` tinyint NOT NULL DEFAULT '0',
  `id` INTEGER PRIMARY KEY AUTOINCREMENT,
  `npcId` smallint NOT NULL DEFAULT '0',
  `x` mediumint NOT NULL DEFAULT '0',
  `y` mediumint NOT NULL DEFAULT '0',
  `z` mediumint NOT NULL DEFAULT '0',
  `heading` mediumint NOT NULL DEFAULT '0',
  `respawnDelay` mediumint NOT NULL DEFAULT '0',
  `isHired` tinyint NOT NULL DEFAULT '1'
) ;
CREATE INDEX IF NOT EXISTS `id` ON `fort_siege_guards` (`fortId`);