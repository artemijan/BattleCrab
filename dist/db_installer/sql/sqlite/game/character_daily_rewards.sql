DROP TABLE IF EXISTS `character_daily_rewards`;
CREATE TABLE IF NOT EXISTS `character_daily_rewards` (
  `charId`  int NOT NULL ,
  `rewardId`  int NOT NULL ,
  `status`  tinyint NOT NULL DEFAULT 1 ,
  `progress`  int NOT NULL DEFAULT 0 ,
  `lastCompleted`  bigint NOT NULL ,
  PRIMARY KEY (`charId`, `rewardId`)
) ;