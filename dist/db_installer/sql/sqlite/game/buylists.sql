DROP TABLE IF EXISTS `buylists`;
CREATE TABLE IF NOT EXISTS `buylists` (
	`buylist_id` INT,
	`item_id` INT,
	`count` BIGINT NOT NULL DEFAULT 0,
	`next_restock_time` BIGINT NOT NULL DEFAULT 0,
	PRIMARY KEY (`buylist_id`, `item_id`)
) ;