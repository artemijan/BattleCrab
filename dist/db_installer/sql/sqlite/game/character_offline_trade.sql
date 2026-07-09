DROP TABLE IF EXISTS `character_offline_trade`;
CREATE TABLE IF NOT EXISTS `character_offline_trade` (
  `charId` int NOT NULL,
  `time` bigint NOT NULL DEFAULT '0',
  `type` tinyint NOT NULL DEFAULT '0',
  `title` varchar(50) DEFAULT NULL,
  PRIMARY KEY (`charId`)
) ;