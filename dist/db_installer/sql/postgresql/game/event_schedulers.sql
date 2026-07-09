DROP TABLE IF EXISTS `event_schedulers`;
CREATE TABLE IF NOT EXISTS `event_schedulers` (
  `id` SERIAL,
  `eventName` varchar(255) COLLATE utf8_unicode_ci NOT NULL,
  `schedulerName` varchar(255) COLLATE utf8_unicode_ci NOT NULL,
  `lastRun` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (`id`)
) ;
CREATE UNIQUE INDEX IF NOT EXISTS `eventName_schedulerName_unique` ON `event_schedulers` (`eventName`,`schedulerName`);