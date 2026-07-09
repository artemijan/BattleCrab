DROP TABLE IF EXISTS `character_tpbookmark`;
CREATE TABLE IF NOT EXISTS `character_tpbookmark` (
  `charId` int NOT NULL,
  `Id` int NOT NULL,
  `x` int NOT NULL,
  `y` int NOT NULL,
  `z` int NOT NULL,
  `icon` int NOT NULL,
  `tag` varchar(50) DEFAULT NULL,
  `name` varchar(50) NOT NULL,
  PRIMARY KEY (`charId`,`Id`)
) ;