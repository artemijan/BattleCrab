DROP TABLE IF EXISTS `character_subclasses`;
CREATE TABLE IF NOT EXISTS `character_subclasses` (
  `charId` INT NOT NULL DEFAULT 0,
  `class_id` int NOT NULL DEFAULT 0,
  `exp` bigint NOT NULL DEFAULT 0,
  `sp` bigint NOT NULL DEFAULT 0,
  `level` int NOT NULL DEFAULT 40,
  `vitality_points` MEDIUMINT NOT NULL DEFAULT 0,
  `class_index` int NOT NULL DEFAULT 0,
  `dual_class` BOOLEAN NOT NULL DEFAULT FALSE,
  PRIMARY KEY (`charId`,`class_id`)
) ;

# RESTORE_CHAR_SUBCLASSES, ADD_CHAR_SUBCLASS, UPDATE_CHAR_SUBCLASS, DELETE_CHAR_SUBCLASS
CREATE INDEX idx_charId_classIndex ON character_subclasses (charId, class_index);

# CharSelectionInfo
CREATE INDEX idx_charId_classId ON character_subclasses (charId, class_id);
