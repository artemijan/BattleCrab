DROP TABLE IF EXISTS `character_friends`;
CREATE TABLE IF NOT EXISTS `character_friends` (
  `charId` INT UNSIGNED NOT NULL DEFAULT 0,
  `friendId` INT UNSIGNED NOT NULL DEFAULT 0,
  `relation` INT UNSIGNED NOT NULL DEFAULT 0,
  `memo` varchar(255) DEFAULT NULL,
  PRIMARY KEY (`charId`,`friendId`)
) DEFAULT CHARSET=utf8 COLLATE=utf8_unicode_ci;