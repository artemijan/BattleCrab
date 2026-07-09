DROP TABLE IF EXISTS `character_friends`;
CREATE TABLE IF NOT EXISTS `character_friends` (
  `charId` INT NOT NULL DEFAULT 0,
  `friendId` INT NOT NULL DEFAULT 0,
  `relation` INT NOT NULL DEFAULT 0,
  `memo` varchar(255) DEFAULT NULL,
  PRIMARY KEY (`charId`,`friendId`)
) ;