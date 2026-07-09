DROP TABLE IF EXISTS `clan_data`;
CREATE TABLE IF NOT EXISTS `clan_data` (
  `clan_id` INT NOT NULL DEFAULT 0,
  `clan_name` varchar(45) ,
  `clan_level` INT,
  `reputation_score` INT NOT NULL DEFAULT 0,
  `hasCastle` INT,
  `blood_alliance_count` smallint NOT NULL DEFAULT 0,
  `blood_oath_count` smallint NOT NULL DEFAULT 0,
  `ally_id` INT,
  `ally_name` varchar(45),
  `leader_id` INT,
  `crest_id` INT,
  `crest_large_id` INT,
  `ally_crest_id` INT,
  `auction_bid_at` INT NOT NULL DEFAULT 0,
  `ally_penalty_expiry_time` bigint NOT NULL DEFAULT '0',
  `ally_penalty_type` tinyint NOT NULL DEFAULT 0,
  `char_penalty_expiry_time` bigint NOT NULL DEFAULT '0',
  `dissolving_expiry_time` bigint NOT NULL DEFAULT '0',
  `new_leader_id` INT NOT NULL DEFAULT '0',
  PRIMARY KEY (`clan_id`)
) ;
CREATE INDEX IF NOT EXISTS `ally_id` ON `clan_data` (`ally_id`);
CREATE INDEX IF NOT EXISTS `leader_id` ON `clan_data` (`leader_id`);
CREATE INDEX IF NOT EXISTS `auction_bid_at` ON `clan_data` (`auction_bid_at`);