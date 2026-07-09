DROP TABLE IF EXISTS `castle_manor_production`;
CREATE TABLE IF NOT EXISTS `castle_manor_production` (
 `castle_id` TINYINT NOT NULL DEFAULT '0',
 `seed_id` INT NOT NULL DEFAULT '0',
 `amount` INT NOT NULL DEFAULT '0',
 `start_amount` INT NOT NULL DEFAULT '0',
 `price` INT NOT NULL DEFAULT '0',
 `next_period` TINYINT NOT NULL DEFAULT '1',
 PRIMARY KEY (`castle_id`, `seed_id`, `next_period`)
) ;