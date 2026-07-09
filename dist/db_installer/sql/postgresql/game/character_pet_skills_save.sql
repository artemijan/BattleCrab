DROP TABLE IF EXISTS `character_pet_skills_save`;
CREATE TABLE IF NOT EXISTS `character_pet_skills_save` (
  `petObjItemId` INT NOT NULL DEFAULT 0,
  `skill_id` INT NOT NULL DEFAULT 0,
  `skill_level` INT NOT NULL DEFAULT 1,
  `skill_sub_level` INT NOT NULL DEFAULT '0',
  `remaining_time` INT NOT NULL DEFAULT 0,
  `buff_index` INT NOT NULL DEFAULT 0,
  PRIMARY KEY (`petObjItemId`,`skill_id`,`skill_level`)
) ;