DROP TABLE IF EXISTS `castle_manor_procure`;
CREATE TABLE IF NOT EXISTS `castle_manor_procure` (
 `castle_id` TINYINT NOT NULL DEFAULT '0',
 `crop_id` INT NOT NULL DEFAULT '0',
 `amount` INT NOT NULL DEFAULT '0',
 `start_amount` INT NOT NULL DEFAULT '0',
 `price` INT NOT NULL DEFAULT '0',
 `reward_type` TINYINT NOT NULL DEFAULT '0',
 `next_period` TINYINT NOT NULL DEFAULT '1',
  PRIMARY KEY (`castle_id`,`crop_id`,`next_period`)
) ;