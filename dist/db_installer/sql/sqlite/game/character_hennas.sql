DROP TABLE IF EXISTS `character_hennas`;
CREATE TABLE IF NOT EXISTS `character_hennas` (
  `charId` INT NOT NULL DEFAULT 0,
  `symbol_id` INT,
  `slot` INT NOT NULL DEFAULT 0,
  `class_index` INT NOT NULL DEFAULT 0,
  PRIMARY KEY (`charId`,`slot`,`class_index`)
) ;

-- RESTORE_CHAR_HENNAS, ADD_CHAR_HENNA, DELETE_CHAR_HENNAS
CREATE INDEX IF NOT EXISTS idx_charId_classIndex ON character_hennas (charId, class_index);

-- DELETE_CHAR_HENNA
CREATE INDEX IF NOT EXISTS idx_charId_slot_classIndex ON character_hennas (charId, slot, class_index);
