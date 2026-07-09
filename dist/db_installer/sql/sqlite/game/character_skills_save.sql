DROP TABLE IF EXISTS `character_skills_save`;
CREATE TABLE IF NOT EXISTS `character_skills_save` (
  `charId` INT NOT NULL DEFAULT 0,
  `skill_id` INT NOT NULL DEFAULT 0,
  `skill_level` INT NOT NULL DEFAULT 1,
  `skill_sub_level` INT NOT NULL DEFAULT '0',
  `remaining_time` INT NOT NULL DEFAULT 0,
  `reuse_delay` INT NOT NULL DEFAULT 0,
  `systime` bigint NOT NULL DEFAULT '0',
  `restore_type` INT NOT NULL DEFAULT 0,
  `class_index` INT NOT NULL DEFAULT 0,
  `buff_index` INT NOT NULL DEFAULT 0,
  PRIMARY KEY (`charId`,`skill_id`,`skill_level`,`class_index`)
) ;

-- ADD_SKILL_SAVE, DELETE_SKILL_SAVE
CREATE INDEX IF NOT EXISTS idx_charId_classIndex ON character_skills_save (charId, class_index);

-- RESTORE_SKILL_SAVE
CREATE INDEX IF NOT EXISTS idx_charId_classIndex_buffIndex ON character_skills_save (charId, class_index, buff_index);
