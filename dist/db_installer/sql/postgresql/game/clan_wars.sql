DROP TABLE IF EXISTS `clan_wars`;
CREATE TABLE IF NOT EXISTS `clan_wars` (
  `clan1` varchar(35) NOT NULL DEFAULT '',
  `clan2` varchar(35) NOT NULL DEFAULT '',
  `clan1Kill` int NOT NULL DEFAULT 0,
  `clan2Kill` int NOT NULL DEFAULT 0,
  `winnerClan` varchar(35) NOT NULL DEFAULT '0',
  `startTime` bigint NOT NULL DEFAULT 0,
  `endTime` bigint NOT NULL DEFAULT 0,
  `state` tinyint NOT NULL DEFAULT 0,
  PRIMARY KEY (`clan1`,`clan2`)
) ;