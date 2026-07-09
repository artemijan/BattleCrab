DROP TABLE IF EXISTS `olympiad_nobles`;
CREATE TABLE IF NOT EXISTS `olympiad_nobles` (
  `charId` int NOT NULL DEFAULT 0,
  `class_id` tinyint NOT NULL DEFAULT 0,
  `olympiad_points` int NOT NULL DEFAULT 0,
  `competitions_done` smallint NOT NULL DEFAULT 0,
  `competitions_won` smallint NOT NULL DEFAULT 0,
  `competitions_lost` smallint NOT NULL DEFAULT 0,
  `competitions_drawn` smallint NOT NULL DEFAULT 0,
  `competitions_done_week` tinyint NOT NULL DEFAULT 0,
  PRIMARY KEY (`charId`)
) ;