DROP TABLE IF EXISTS `character_reco_bonus`;
CREATE TABLE IF NOT EXISTS `character_reco_bonus` (
  `charId` int NOT NULL,
  `rec_have` tinyint NOT NULL DEFAULT '0',
  `rec_left` tinyint NOT NULL DEFAULT '0',
  `time_left` bigint NOT NULL DEFAULT '0'
) ;
CREATE UNIQUE INDEX IF NOT EXISTS `charId_unique` ON `character_reco_bonus` (`charId`);