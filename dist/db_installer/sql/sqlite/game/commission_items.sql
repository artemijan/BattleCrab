DROP TABLE IF EXISTS `commission_items`;
CREATE TABLE IF NOT EXISTS `commission_items` (
	`commission_id` INTEGER PRIMARY KEY AUTOINCREMENT,
	`item_object_id` INT NOT NULL,
	`price_per_unit` BIGINT NOT NULL,
	`start_time` TIMESTAMP NOT NULL,
	`duration_in_days` TINYINT NOT NULL,
	`discount_in_percentage` TINYINT NOT NULL
) ;