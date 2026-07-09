DROP TABLE IF EXISTS `character_summons`;
CREATE TABLE IF NOT EXISTS `character_summons` (
  `ownerId` int NOT NULL,
  `summonId` int NOT NULL,
  `summonSkillId` int NOT NULL,
  `curHp` int DEFAULT '0',
  `curMp` int DEFAULT '0',
  `time` int NOT NULL DEFAULT '0',
  PRIMARY KEY (`ownerId`,`summonId`,`summonSkillId`)
) ;