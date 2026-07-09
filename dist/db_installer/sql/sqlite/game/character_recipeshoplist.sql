DROP TABLE IF EXISTS `character_recipeshoplist`;
CREATE TABLE IF NOT EXISTS `character_recipeshoplist` (
  `charId` int NOT NULL DEFAULT 0,
  `recipeId` int NOT NULL DEFAULT 0,
  `price` bigint NOT NULL DEFAULT 0,
  `index` tinyint NOT NULL DEFAULT 0,
  PRIMARY KEY (`charId`,`recipeId`)
) ;