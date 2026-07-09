DROP TABLE IF EXISTS `party_matching_history`;
CREATE TABLE `party_matching_history` (
  `id` SERIAL,
  `title` VARCHAR(21) DEFAULT NULL,
  `leader` VARCHAR(35) DEFAULT NULL,
  PRIMARY KEY (`id`)
) ;