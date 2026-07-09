DROP TABLE IF EXISTS `heroes_diary`;
CREATE TABLE IF NOT EXISTS `heroes_diary` (
  `charId` int NOT NULL,
  `time` bigint NOT NULL DEFAULT '0',
  `action` tinyint NOT NULL DEFAULT '0',
  `param` int NOT NULL DEFAULT '0'
) ;
CREATE INDEX IF NOT EXISTS `charId` ON `heroes_diary` (`charId`);