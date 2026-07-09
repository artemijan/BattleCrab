DROP TABLE IF EXISTS `castle_trapupgrade`;
CREATE TABLE IF NOT EXISTS `castle_trapupgrade` (
  `castleId` tinyint NOT NULL DEFAULT '0',
  `towerIndex` tinyint NOT NULL DEFAULT '0',
  `level` tinyint NOT NULL DEFAULT '0',
  PRIMARY KEY (`towerIndex`,`castleId`)
) ;