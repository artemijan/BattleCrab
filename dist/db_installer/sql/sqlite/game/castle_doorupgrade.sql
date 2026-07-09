DROP TABLE IF EXISTS `castle_doorupgrade`;
CREATE TABLE IF NOT EXISTS `castle_doorupgrade` (
  `doorId` int NOT NULL DEFAULT '0',
  `ratio` tinyint NOT NULL DEFAULT '0',
  `castleId` tinyint NOT NULL DEFAULT '0',
  PRIMARY KEY (`doorId`)
) ;