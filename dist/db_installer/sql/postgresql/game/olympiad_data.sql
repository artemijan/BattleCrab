DROP TABLE IF EXISTS `olympiad_data`;
CREATE TABLE IF NOT EXISTS `olympiad_data` (
  `id` TINYINT NOT NULL DEFAULT 0,
  `current_cycle` MEDIUMINT NOT NULL DEFAULT 1,
  `period` MEDIUMINT NOT NULL DEFAULT 0,
  `olympiad_end` bigint NOT NULL DEFAULT '0',
  `validation_end` bigint NOT NULL DEFAULT '0',
  `next_weekly_change` bigint NOT NULL DEFAULT '0',
  PRIMARY KEY (`id`)
) ;