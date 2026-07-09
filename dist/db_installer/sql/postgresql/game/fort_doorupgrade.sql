DROP TABLE IF EXISTS `fort_doorupgrade`;
CREATE TABLE IF NOT EXISTS `fort_doorupgrade` (
  `doorId` int NOT NULL DEFAULT '0',
  `fortId` int NOT NULL,
  `hp` int NOT NULL DEFAULT '0',
  `pDef` int NOT NULL DEFAULT '0',
  `mDef` int NOT NULL DEFAULT '0',
  PRIMARY KEY (`doorId`)
) ;