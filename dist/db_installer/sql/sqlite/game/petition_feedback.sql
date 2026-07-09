DROP TABLE IF EXISTS `petition_feedback`;
CREATE TABLE IF NOT EXISTS `petition_feedback` (
  `charName` VARCHAR(35) NOT NULL,
  `gmName`  VARCHAR(35) NOT NULL,
  `rate` TINYINT NOT NULL DEFAULT 2,
  `message` text NOT NULL,
  `date` bigint NOT NULL DEFAULT '0'
) ;