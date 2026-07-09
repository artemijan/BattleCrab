DROP TABLE IF EXISTS `character_instance_time`;
CREATE TABLE IF NOT EXISTS `character_instance_time` (
  `charId` INT NOT NULL DEFAULT '0',
  `instanceId` int NOT NULL DEFAULT '0',
  `time` bigint NOT NULL DEFAULT '0',
  PRIMARY KEY (`charId`,`instanceId`)
) ;