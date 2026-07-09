DROP TABLE IF EXISTS `olympiad_fights`;
CREATE TABLE IF NOT EXISTS `olympiad_fights` (
  `charOneId` int NOT NULL,
  `charTwoId` int NOT NULL,
  `charOneClass` tinyint NOT NULL DEFAULT '0',
  `charTwoClass` tinyint NOT NULL DEFAULT '0',
  `winner` tinyint NOT NULL DEFAULT '0',
  `start` bigint NOT NULL DEFAULT '0',
  `time` bigint NOT NULL DEFAULT '0',
  `classed` tinyint NOT NULL DEFAULT 0,
  KEY `charOneId` (`charOneId`),
  KEY `charTwoId` (`charTwoId`)
) ;
CREATE INDEX IF NOT EXISTS `charOneId` ON `olympiad_fights` (`charOneId`);
CREATE INDEX IF NOT EXISTS `charTwoId` ON `olympiad_fights` (`charTwoId`);