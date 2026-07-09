DROP TABLE IF EXISTS `cursed_weapons`;
CREATE TABLE IF NOT EXISTS `cursed_weapons` (
  `itemId` INT,
  `charId` INT NOT NULL DEFAULT 0,
  `playerReputation` INT DEFAULT 0,
  `playerPkKills` INT DEFAULT 0,
  `nbKills` INT DEFAULT 0,
  `endTime` bigint NOT NULL DEFAULT '0',
  PRIMARY KEY (`itemId`),
  KEY `charId` (`charId`)
) ;
CREATE INDEX IF NOT EXISTS `charId` ON `cursed_weapons` (`charId`);