DROP TABLE IF EXISTS `character_quests`;
CREATE TABLE IF NOT EXISTS `character_quests` (
  `charId` INT NOT NULL DEFAULT 0,
  `name` VARCHAR(60) NOT NULL DEFAULT '',
  `var`  VARCHAR(20) NOT NULL DEFAULT '',
  `value` VARCHAR(255) ,
  PRIMARY KEY (`charId`,`name`,`var`)
) ;

CREATE INDEX IF NOT EXISTS idx_charId_name ON character_quests (charId, name);
CREATE INDEX IF NOT EXISTS idx_charId_var ON character_quests (charId, var);
CREATE UNIQUE INDEX IF NOT EXISTS idx_charId_name_var ON character_quests (charId, name, var);
